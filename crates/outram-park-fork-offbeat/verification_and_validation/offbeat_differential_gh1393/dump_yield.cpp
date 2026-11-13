// SPDX-License-Identifier: GPL-3.0-only
//
// Dump driver 6 for gh:#1393 round 2: OFFBEAT 80e84450a115b0c411e1bfa5d166379f6bf6c084
// `yieldStressFRAPTRAN::correctYieldStress`, cell part (the boundary-face part
// repeats the same formulas on patch values), transcribed.  Same route and
// caveats as dump_conductivity.cpp.  Three variants per grid point:
//   Yield_intended : the `>=` fluence chain for K that the port implements
//                    (the port's documented "intended" reading);
//   Yield_lit1     : upstream as written, first call (phiValue_ = phi * 1e25);
//   Yield_lit2     : upstream as written, second call (phiValue_ = phi * 1e25 * 1e25),
//                    because `phiValue_ *= 1e25;` is executed on every call.
// Output: name T phi_1e25 CW epsOld rate E scale value
//
// Build: g++ -O2 -ffp-contract=off -o dump_yield dump_yield.cpp
#include <cmath>
#include <cstdio>
#include <algorithm>
#include <initializer_list>

typedef double scalar;
using std::pow; using std::exp;
// OpenFOAM's min/max(scalar, scalar) accept the int literal in `min(1, exp(..))`
static scalar min(scalar a, scalar b) { return (b < a) ? b : a; }
static scalar max(scalar a, scalar b) { return (a < b) ? b : a; }
static scalar A_(scalar x) { return std::fabs(x); }

struct Coef { scalar m, n, K, nabs, nmult, kabs, kpoly; };

// variant: 0 intended, 1 literal
static Coef coefficients(scalar T, scalar phiValue_, scalar CWValue_, int variant)
{
    Coef c; scalar m_ = 0, n_ = 0, Kcoeff_ = 0;
    c.nabs = 0; c.kabs = 0; c.kpoly = 0;
        if(T < 750)
        {
            m_ = 0.015;                                                  // UPSTREAM
        }
        if(750 <= T && T <= 800)
        {
            m_ = 7.458e-4*T - 0.544338;                                  // UPSTREAM
        }
        if(T > 800)
        {
            m_ = 3.24124e-4*T - 0.20701;                                 // UPSTREAM
        }
        if(T < 419.4)
        {
            n_ = 0.11405;                                                // UPSTREAM
            c.nabs = 0.11405;
        }
        else if(419.4 <= T && T <= 1099.0722)
        {
            n_ = -9.490e-2 + 1.165e-3*T - 1.992e-6*pow(T, 2) + 9.558e-10*pow(T, 3); // UPSTREAM
            c.nabs = 9.490e-2 + 1.165e-3*T + 1.992e-6*T*T + 9.558e-10*T*T*T;
        }
        else if(1099.0722 <= T && T <= 1600)
        {
            n_ = -0.22655119 + 2.5e-4*T;                                 // UPSTREAM
            c.nabs = 0.22655119 + 2.5e-4*T;
        }
        else if(T > 1600)
        {
            n_ = 0.17344880;                                             // UPSTREAM
            c.nabs = 0.17344880;
        }
        c.nmult = 1;
        if(phiValue_ < 0.1e25){
            n_ *= 1.321 + 0.48e-25*phiValue_;                            // UPSTREAM
            c.nmult = 1.321 + 0.48e-25*phiValue_;
        }
        else if(0.1e25 <= phiValue_ && phiValue_ < 2e25){
            n_ *= 1.369 + 0.096e-25*phiValue_;                           // UPSTREAM
            c.nmult = 1.369 + 0.096e-25*phiValue_;
        }
        else if(2e25 <= phiValue_ && phiValue_ < 7.5e25){
            n_ *= 1.5435 + 0.008727e-25*phiValue_;                       // UPSTREAM
            c.nmult = 1.5435 + 0.008727e-25*phiValue_;
        }
        else if(phiValue_ >= 7.5e25){
            n_ *= 1.608953;                                              // UPSTREAM
            c.nmult = 1.608953;
        }
    const scalar KCW = 0.546 * CWValue_;                                 // UPSTREAM
    scalar Kphi = 0;
        if(T < 750)
        {
            Kcoeff_ = 1.17628e9 + 4.54859e5*T - 3.28185e3*pow(T, 2) + 1.72752*pow(T,3); // UPSTREAM
            c.kabs = 1.17628e9 + 4.54859e5*T + 3.28185e3*T*T + 1.72752*T*T*T;
        }
        else if(750 <= T && T < 1090)
        {
            Kcoeff_ = 2.522488e6 * exp(2.8500027e6/pow(T, 2));           // UPSTREAM
            c.kabs = Kcoeff_ * (1 + 2.8500027e6/(T*T));
        }
        else if(1090 <= T && T < 1255)
        {
            Kcoeff_ = 1.841376039e8 -  1.4345448e5*T;                    // UPSTREAM
            c.kabs = 1.841376039e8 + 1.4345448e5*T;
        }
        else if(1255 <= T && T < 2100)
        {
            Kcoeff_ = 4.330e7 - 6.685e4*T + 37.579*pow(T, 2) - 7.33e-3*pow(T, 3); // UPSTREAM
            c.kabs = 4.330e7 + 6.685e4*T + 37.579*T*T + 7.33e-3*T*T*T;
        }
    c.kpoly = Kcoeff_;
    if (variant == 1)
    {
        // upstream as written
        if(phiValue_ < 0.1e25){
            Kphi = -0.1461 + 1.464e-25*phiValue_ * ((2.25 * exp(-20*CWValue_) * min(1, exp((T-550)/10))) + 1); // UPSTREAM
            Kcoeff_ *= 1 + KCW + Kphi;                                   // UPSTREAM
        }
        else if(phiValue_ <= 0.1e25 && phiValue_ < 2e25){
            Kphi = 2.928e-26*phiValue_;                                  // UPSTREAM
            Kcoeff_ *= 1 + KCW + Kphi;                                   // UPSTREAM
        }
        else if(phiValue_ <= 2e25 && phiValue_ < 12e25){
            Kphi = 0.53236 + 2.6618e-27*phiValue_;                       // UPSTREAM
            Kcoeff_ *= 1 + KCW + Kphi;                                   // UPSTREAM
        }
    }
    else
    {
        // the evidently intended chain (>= comparisons), as the port documents it
        if(phiValue_ < 0.1e25){
            Kphi = -0.1461 + 1.464e-25*phiValue_ * ((2.25 * exp(-20*CWValue_) * min(1, exp((T-550)/10))) + 1);
        }
        else if(phiValue_ < 2e25){
            Kphi = 2.928e-26*phiValue_;
        }
        else if(phiValue_ < 12e25){
            Kphi = 0.53236 + 2.6618e-27*phiValue_;
        }
        else {
            Kphi = 0.53236 + 2.6618e-27*12e25;
        }
        Kcoeff_ *= 1 + KCW + Kphi;
    }
    c.m = m_; c.n = n_; c.K = Kcoeff_;
    return c;
}

static scalar sigmaY(const Coef& c, scalar Ei, scalar epsilonEqOld, scalar ddtEpsilonEq_in, scalar* scale)
{
    scalar ddtEpsilonEq = max(ddtEpsilonEq_in, 1e-3);                    // (max(mag(..)/dt, 1e-3))
    scalar term1 = c.K / pow(Ei, c.n) * pow(ddtEpsilonEq/1e-3, c.m);     // UPSTREAM
    scalar term2 = 1/(1-c.n);                                            // UPSTREAM
    scalar sigmaYOld = c.K*pow(epsilonEqOld, c.n) * pow(ddtEpsilonEq/1e-3, c.m); // UPSTREAM
    const scalar s0 = pow(term1, term2);
    const scalar sY = max(s0, sigmaYOld);                                // UPSTREAM (sigmaY_[cellI] = max(pow(term1, term2), sigmaYOld))
    const scalar lnr = std::log(ddtEpsilonEq / 1e-3);
    const scalar condK = c.kabs / A_(c.kpoly) + 1.0;
    scalar cond;
    if (s0 >= sigmaYOld)
    {
        const scalar dlnsdn = (std::log(c.K) - std::log(Ei) + c.m * lnr) / ((1 - c.n) * (1 - c.n));
        cond = condK / (1 - c.n) + A_(dlnsdn) * c.nabs * c.nmult + 2.0;
    }
    else
    {
        cond = condK + A_(std::log(epsilonEqOld)) * c.nabs * c.nmult + 2.0;
    }
    *scale = A_(sY) * cond;
    return sY;
}

static void pr(const char* name, std::initializer_list<scalar> c, scalar scale, scalar v)
{
    std::printf("%s", name);
    for (scalar x : c) std::printf(" %.17g", x);
    std::printf(" %.17g %.17g\n", scale, v);
}

int main()
{
    const scalar E = 1e11;
    for (scalar T : {300.0, 419.4, 500.0, 650.0, 749.0, 750.0, 775.0, 800.0, 900.0, 1089.0, 1090.0, 1099.0722,
                     1200.0, 1254.0, 1255.0, 1500.0, 1600.0, 1700.0, 2000.0})
        for (scalar phiIn : {0.0, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0, 7.5, 10.0, 12.0, 15.0})
            for (scalar CW : {0.0, 0.5})
                for (scalar eo : {0.0, 0.01, 0.05})
                    for (scalar rate : {1e-3, 5e-3, 0.1})
                    {
                        scalar sc, v;
                        scalar phi1 = phiIn * 1e25;            // first call:  phiValue_ *= 1e25
                        scalar phi2 = phi1 * 1e25;             // second call: phiValue_ *= 1e25 again
                        Coef ci = coefficients(T, phi1, CW, 0);
                        v = sigmaY(ci, E, eo, rate, &sc);  pr("Yield_intended", {T, phiIn, CW, eo, rate, E}, sc, v);
                        Coef c1 = coefficients(T, phi1, CW, 1);
                        v = sigmaY(c1, E, eo, rate, &sc);  pr("Yield_lit1", {T, phiIn, CW, eo, rate, E}, sc, v);
                        Coef c2 = coefficients(T, phi2, CW, 1);
                        v = sigmaY(c2, E, eo, rate, &sc);  pr("Yield_lit2", {T, phiIn, CW, eo, rate, E}, sc, v);
                    }
    return 0;
}
