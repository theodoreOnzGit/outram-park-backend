// SPDX-License-Identifier: GPL-3.0-only
//
// Dump driver 4 for gh:#1393 round 2: OFFBEAT 80e84450a115b0c411e1bfa5d166379f6bf6c084
// densification (Empirical, FRAPCON), relocation (FRAPCON `setRelocation`) and
// swelling (Constant, FRAPCON, FrCrAl, AIM1 / Generalized 15-15Ti growth,
// WrightSham Hastelloy N, BISON Zircaloy growth, MATPRO UO2, FBR-MOX,
// correlation PyC) kernels, transcribed into a standalone program.  Same route
// and caveats as dump_conductivity.cpp (statements marked UPSTREAM are copied
// verbatim from the `correct()` bodies and checked by check_verbatim.sh; member
// look-ups `Bu_->internalField()[cellI]` etc. are replaced by locals; F = 1,
// delta = 0 defaults; `oldTime()` fields are loop variables).
//
// Output columns: name <inputs...> scale value (the last two are scale and the
// upstream value; the inputs differ per model, see main()).
//
// Build: g++ -O2 -ffp-contract=off -o dump_behavioral dump_behavioral.cpp
#include <cmath>
#include <cstdio>
#include <algorithm>

typedef double scalar;
using std::pow; using std::exp; using std::min; using std::max; using std::abs; using std::log;
namespace Foam { inline double log(double x) { return std::log(x); } }
namespace constant { namespace mathematical { const double pi = M_PI; } }
static scalar A_(scalar x) { return std::fabs(x); }

// ---- densificationEmpirical.C --------------------------------------------
static scalar densEmpirical(scalar Bu, scalar densityFrac_, scalar densificationDensityChange_,
                            scalar densificationTimeConstant_, scalar* scale)
{
    scalar oldTime = 0.0;
        scalar dLOverL_max(-densificationDensityChange_/(                // UPSTREAM
            densityFrac_*100 + densificationDensityChange_)/3);          // UPSTREAM
        scalar Bui = Bu/0.881;                                           // (Bu_->internalField()[cellI]/0.881)
        scalar nominalValue =                                            // UPSTREAM
        dLOverL_max*(1 - exp(-Bui/densificationTimeConstant_));          // UPSTREAM
        nominalValue = nominalValue * 1.0 + 0.0;                         // (F = 1, delta = 0)
    *scale = A_(dLOverL_max);
    return min(nominalValue, oldTime);
}

// ---- densificationFRAPCON.C -----------------------------------------------
static scalar densFrapcon(scalar Ti, scalar Bu, scalar densityFrac_, scalar resintDensChange_,
                          scalar Tsinter_, int* iters, scalar* scale)
{
    scalar par1 = 22.2, par2 = 1453, par3 = 66.6, par4 = 2.0, par5 = 35, par6 = 3.0;
    scalar oldTime = 0.0;
        double dLOverL_max(0.0);
        const scalar Bui = Bu/1000/0.881;                                // (Bu_->internalField()[cellI]/1000/0.881)
        if(resintDensChange_ == 0)
        {
            if(Ti < 950)
            {
                dLOverL_max =                                            // UPSTREAM
                -par1*(1 - densityFrac_)/(Tsinter_ - par2);              // UPSTREAM
            }
            else if(Ti < 1050)
            {
                scalar dLOverL_950 =                                     // UPSTREAM
                -par1*(1 - densityFrac_)/(Tsinter_ - par2);              // UPSTREAM
                scalar dLOverL_1050 =                                    // UPSTREAM
                -par1*(1 - densityFrac_)/(Tsinter_ - par2);              // UPSTREAM
                dLOverL_max = dLOverL_950 +                              // UPSTREAM
                (Ti - 950)/(1050 - 950)*(dLOverL_1050 - dLOverL_950);    // UPSTREAM
            }
            else
            {
                dLOverL_max =                                            // UPSTREAM
                -par3*(1 - densityFrac_)/(Tsinter_ - par2);              // UPSTREAM
            }
        }
        else
        {
            dLOverL_max =                                                // UPSTREAM
            -resintDensChange_/(densityFrac_*100 + resintDensChange_)/3; // UPSTREAM
            if(Ti < 950)
            {
                dLOverL_max = -(0.0015*resintDensChange_*10960/100)/100; // UPSTREAM
            }
            else if(Ti < 1050)
            {
                scalar dLOverL_950 =                                     // UPSTREAM
                -(0.0015*resintDensChange_*10960/100)/100;;              // UPSTREAM
                scalar dLOverL_1050 =                                    // UPSTREAM
                -(0.00285*resintDensChange_*10960/100)/100;              // UPSTREAM
                dLOverL_max = dLOverL_950 +                              // UPSTREAM
                (Ti - 950)/(1050 - 950)*(dLOverL_1050 - dLOverL_950);    // UPSTREAM
            }
            else
            {
                dLOverL_max = -(0.00285*resintDensChange_*10960/100)/100; // UPSTREAM
            }
        }
        double B0, B = 1;
        int nIters = 0;
        B = 1;
        do
        {
            B0 = B;
            B = -log(-par4*exp(-par5*B) - dLOverL_max)/par6;             // UPSTREAM
            nIters++;
            if (nIters > 10)
            {
                *iters = 99; *scale = 1.0; return 0.0 / 0.0;
            }
        } while (abs(B-B0) > 1e-6);                                      // UPSTREAM
        scalar nominalValue =                                            // UPSTREAM
        dLOverL_max + exp(-par6*(Bui+B)) + par4*exp(-par5*(Bui+B));      // UPSTREAM
        nominalValue = nominalValue * 1.0 + 0.0;                         // (F = 1, delta = 0)
    *iters = nIters;
    const scalar s = Bui + B;
    *scale = A_(dLOverL_max) + exp(-par6*s) * (1 + par6 * s) + par4 * exp(-par5*s) * (1 + par5 * s);
    return min(nominalValue, oldTime);
}

// ---- relocationFRAPCON.C : setRelocation ----------------------------------
static scalar setRelocation(double Q, double Gap, double D, double Buslice, bool modifiedRelocationModel_,
                            scalar* scale)
{
    double Bu = Buslice/1000/0.881;                                      // UPSTREAM
    scalar Qprime = Q*constant::mathematical::pi*pow(D/2.0,2.0)/1000;    // UPSTREAM
    scalar deltaGap(0.0);
    scalar sc = 0;
    if(modifiedRelocationModel_)
    {
        if(Bu < 0.0937) // TODO : correction at low burnup or not?
        {
                deltaGap = 0.055*100;                                    // UPSTREAM
                sc = 0.055*100;
        }
        else
        {
            scalar reloc = 0;
            if(Qprime < 20)
            {
                    reloc = 0.345;                                       // UPSTREAM
            }
            else if(Qprime <= 40)
            {
                    reloc = 0.345 + (Qprime - 20.0)/200.0;               // UPSTREAM
            }
            else
            {
                    reloc = 0.445;                                       // UPSTREAM
            }
            deltaGap =                                                   // UPSTREAM
            (0.055 + min(reloc, reloc*(0.5795 + 0.2447*Foam::log(Bu))))*100; // UPSTREAM
            sc = (0.055 + reloc*(0.5795 + 0.2447*A_(std::log(Bu))))*100;
        }
    }
    else
    {
        scalar PFACTOR = (Qprime - 20)*5/20;                             // UPSTREAM
        scalar FBu = 0;
        if(Bu < 5)
        {
                FBu = Bu/5;                                              // UPSTREAM
        }
        else
        {
                FBu = 1;                                                 // UPSTREAM
        }
        if(Qprime < 20 + 1e-6 )
        {
                deltaGap = (30 + 10*FBu);                                // UPSTREAM
        }
        else if(Qprime < 40 + 1e-6 )
        {
                deltaGap = (28 + PFACTOR + (12 + PFACTOR)*FBu);          // UPSTREAM
        }
        else
        {
                deltaGap = (32 + 18*FBu);                                // UPSTREAM
        }
        sc = deltaGap;
    }
    const scalar nominalValue = deltaGap*(Gap/D)/100;                    // UPSTREAM
    *scale = sc * (Gap/D) / 100;
    return nominalValue;
}

// ---- constantSwelling.C ----------------------------------------------------
static scalar swConstant(scalar s_, scalar fluence_cm2, scalar* scale)
{
        const scalar phi = fluence_cm2*1e4/1e25;                         // (phii[cellI]*1e4/1e25)
        scalar nominalValue(s_*phi);                                     // UPSTREAM
        nominalValue = nominalValue * 1.0 + 0.0;
    *scale = A_(nominalValue);
    return nominalValue;
}

// ---- swellingFrCrAl.C ------------------------------------------------------
static scalar swFrCrAl(scalar par1, scalar fluence_cm2, scalar* scale)
{
        const scalar phi = fluence_cm2*1e4;                              // (phii[cellI]*1e4)
        scalar nominalValue(par1*phi);                                   // UPSTREAM
        nominalValue = nominalValue * 1.0 + 0.0;
    *scale = A_(nominalValue);
    return nominalValue;
}

// ---- swellingFRAPCON.C -----------------------------------------------------
static scalar swFrapcon(scalar Bu, scalar rho, scalar* scale)
{
    scalar par1 = 6000, par2 = 80000, par3 = 2.974e+10, par4 = 2.315e-23, par5 = 86.4, par6 = 3.211e-23;
        const double a1 = par1;                                          // UPSTREAM
        const double a2 = par2;                                          // UPSTREAM
        const double b1 = par3 * par4 * par5;                            // UPSTREAM
        const double b2 = par3 * par6 * par5;                            // UPSTREAM
        const scalar Bui = Bu/0.881;                                     // (BuI[cellI]/0.881)
        scalar nominalValue(0.0);
        if (Bui  < a1)
        {
                nominalValue = 0.0;                                      // UPSTREAM
        }
        else if (Bui < a2)
        {
                nominalValue = (Bui - a1)*b1 * rho/3;                    // UPSTREAM
        }
        else
        {
                nominalValue = ((a2 - a1)*b1 + (Bui - a2)*b2) * rho/3;   // UPSTREAM
        }
        nominalValue = nominalValue * 1.0 + 0.0;
    *scale = Bui * std::max(b1, b2) * rho / 3 + A_(nominalValue);
    return nominalValue;
}

// ---- swellingGrowthAIM11515Ti.C / swellingGrowthGeneralized1515Ti.C -------
static scalar swSteelVoid(scalar T, scalar fluence_cm2, scalar par1, scalar par2, scalar par3, scalar par4,
                          scalar par5, scalar* scale)
{
        const scalar Ti = T - 273.15;                                    // (T[cellI] - 273.15)
        const scalar fastFluence = fluence_cm2*1e-22;                    // (fastFluenceI[cellI]*1e-22)
        const scalar fastFluenceOld = 0.0*1e-22;
        scalar nominalValue = 1.0/3.0 *                                  // UPSTREAM
        par1 * exp(par2*(pow((Ti-par3)/par4,2)))*pow(fastFluence,par5)*1e-2; // UPSTREAM
        const scalar nominalValueOld = 1.0/3.0 *                         // UPSTREAM
        par1 * exp(par2*(pow((Ti-par3)/par4,2)))*pow(fastFluenceOld,par5)*1e-2; // UPSTREAM
        scalar swellIncrement(nominalValue-nominalValueOld);             // UPSTREAM
        swellIncrement = swellIncrement * 1.0 + 0.0;
    const scalar x = (Ti - par3) / par4;
    *scale = A_(swellIncrement) * (1 + A_(par2 * x * x) + par5);
    return swellIncrement;
}

// ---- swellingWrightShamHastelloyN.C ----------------------------------------
static scalar swWrightSham(scalar T, scalar fluence_cm2, scalar* scale)
{
    scalar par1 = 9.845e-1, par2 = 4.385e-1, par3 = -9.81e-1, par4 = 490.;
        const scalar fastFluence = fluence_cm2*1e-22*5;                  // (fastFluenceI[cellI]*1e-22*5)
        const scalar fastFluenceOld = 0.0*1e-22*5;
        const scalar fluxfac    = par1*pow(fastFluence,par2) + par3;     // UPSTREAM
        const scalar fluxfacOld = par1*pow(fastFluenceOld,par2) + par3;  // UPSTREAM
        const scalar Ti = T - 273.15;                                    // (T[cellI] - 273.15)
        const scalar sweexp = (Ti-par4)/100.0;                           // UPSTREAM
        const scalar tfact  = exp(-(sweexp*sweexp));                     // UPSTREAM
        const scalar nominalValue = tfact*fluxfac;                       // UPSTREAM
        const scalar nominalValueOld = tfact*fluxfacOld;                 // UPSTREAM
        scalar swellIncrement = 0.01/3.0*(nominalValue - nominalValueOld); // UPSTREAM
        swellIncrement = swellIncrement * 1.0 + 0.0;
    // fluxfac = par1*f^par2 + par3 : sum of terms of mixed sign
    *scale = 0.01 / 3.0 * tfact * (A_(par1 * pow(fastFluence, par2)) + A_(par3)
                                   + A_(par1 * pow(fastFluenceOld, par2)) + A_(par3))
             * (1 + 2 * sweexp * sweexp);
    return swellIncrement;
}

// ---- swellingGrowthBISONZy.C : N equal fluence steps -----------------------
// type: 0 ESCORE, 1 SRA, 2 RXA, 3 PRA, 4 ZIRLO, 5 M5
static void swZy(int type, scalar fluence_cm2, int N, scalar* xx, scalar* zz, scalar* S)
{
    scalar A = 3e-20, n = 0.794;
    if (type == 1) { A = 2.18e-21; n = 0.845; }                         // UPSTREAM
    if (type == 2) { A = 1.09e-21; n = 0.845; }                         // UPSTREAM
    if (type == 3) { A = 1.09e-21; n = 0.845; }                         // UPSTREAM
    if (type == 4) { A = 9.7893e-25; n = 0.98239; }                     // UPSTREAM
    if (type == 5) { A = 7.013e-21; n = 0.81787; }                      // UPSTREAM
    scalar sxx = 0, szz = 0;
    for (int k = 1; k <= N; k++)
    {
        const scalar fastFluence = (fluence_cm2 * k) / N;
        const scalar fastFluenceOld = (fluence_cm2 * (k - 1)) / N;
        const scalar nominalValue = A*pow(fastFluence,n);                // UPSTREAM
        const scalar nominalValueOld = A*pow(fastFluenceOld,n);          // UPSTREAM
        scalar swellIncrement = nominalValue - nominalValueOld;          // UPSTREAM
        swellIncrement = swellIncrement * 1.0 + 0.0;
        sxx += -(1 - pow((1 + swellIncrement), -0.5));                   // UPSTREAM (swellingI[cellI].xx() += ...)
        szz += (swellIncrement);                                         // UPSTREAM (swellingI[cellI].zz() += ...)
    }
    *xx = sxx; *zz = szz; *S = A * pow(fluence_cm2, n);
}

// ---- swellingMATPRO.C : N equal burnup steps at fixed T and rho -------------
static scalar swMatpro(scalar T, scalar rho, scalar Bu_final, int N, scalar* relbound)
{
    scalar par1 = 5.577e-5, par2 = 1.96e-31, par3 = 2800, par4 = 11.73, par5 = -0.0162, par6 = -0.0178;
    scalar swelling = 0;
    for (int k = 1; k <= N; k++)
    {
        const scalar BuI = (Bu_final * k) / N;
        const scalar BuIOld = (Bu_final * (k - 1)) / N;
        const scalar Bu = BuI*1e-3/0.881/937.06;                         // UPSTREAM (BuI[cellI]*1e-3/0.881/937.06)
        const scalar BuOld = BuIOld*1e-3/0.881/937.06;                   // UPSTREAM
        const scalar solidSwellIncrement = par1*rho*(Bu-BuOld);          // UPSTREAM
        const scalar gasSwellIncrement = par2*rho*(Bu-BuOld)*pow(par3-T,par4) // UPSTREAM
                                        *exp(par5*(par3-T))*exp(par6*rho*Bu); // UPSTREAM
        scalar nominalValue = (solidSwellIncrement+gasSwellIncrement)/3.0; // UPSTREAM
        nominalValue = nominalValue * 1.0 + 0.0;
        swelling = swelling + nominalValue;                              // (swellingI = swellingIOld + nominalValue * I)
    }
    const scalar F = Bu_final*1e-3/0.881/937.06;
    *relbound = A_(par6 * rho) * F / N;
    return swelling;
}

// ---- swellingFBRMOX.C : N equal burnup steps, fixed gap state ----------------
static scalar swFbr(scalar Bu_final, int N, bool gapOpen, scalar* absbound)
{
    scalar par1 = 0.020, par2 = 0.012, par3 = 0.0065;
    scalar swelling = 0;
    for (int k = 1; k <= N; k++)
    {
        const scalar Bu_1D = (Bu_final * k) / N;
        const scalar BuOld_1D = (Bu_final * (k - 1)) / N;
        const scalar BuI = Bu_1D * 1e-3 / 9.5;                           // UPSTREAM
        const scalar BuOldI = BuOld_1D * 1e-3 / 9.5;                     // UPSTREAM
        const scalar deltaBu = BuI-BuOldI;                               // UPSTREAM
        const scalar solidSwellIncrement = par3*deltaBu;                 // UPSTREAM
        scalar totSwellIncrement = 0;
        if ( BuI <= 1 and gapOpen )                                      // UPSTREAM
        {
            totSwellIncrement = par1*deltaBu;                            // UPSTREAM
        }
        else if ( BuI > 1 and gapOpen )                                  // UPSTREAM
        {
            totSwellIncrement = par2*deltaBu;                            // UPSTREAM
        }
        else // gap closed
        {
            totSwellIncrement = solidSwellIncrement;                     // UPSTREAM
        }
        totSwellIncrement = totSwellIncrement * 1.0 + 0.0;
        swelling = swelling + (totSwellIncrement)/3.0;                   // UPSTREAM-ish: swellingIOld + (totSwellIncrement)/3.0 * I
    }
    const scalar Ffinal = Bu_final * 1e-3 / 9.5;
    *absbound = gapOpen ? (par1 - par2) * (Ffinal / N) / 3.0 : 0.0;
    return swelling;
}

// ---- swellingCorrelationPyC.C ------------------------------------------------
static void swPyC(scalar fluence_cm2, scalar fcf_, const scalar* Ar_, const scalar* At_, scalar* er, scalar* et,
                  scalar* scale)
{
        const scalar phi = fluence_cm2*1e4/1e25*fcf_;                    // (phii[cellI]*1e4/1e25*fcf_)
        const scalar epsilon_r = Ar_[0]*phi + Ar_[1]/2.0*phi*phi + Ar_[2]/3.0*pow(phi,3) // UPSTREAM
                               + Ar_[3]/4.0*pow(phi,4) + Ar_[4]/5.0*pow(phi,5)           // UPSTREAM
                               + Ar_[5]/6.0*pow(phi,6);                                  // UPSTREAM
        const scalar epsilon_t = At_[0]*phi + At_[1]/2.0*phi*phi + At_[2]/3.0*pow(phi,3) // UPSTREAM
                               + At_[3]/4.0*pow(phi,4) + At_[4]/5.0*pow(phi,5)           // UPSTREAM
                               + At_[5]/6.0*pow(phi,6);                                  // UPSTREAM
    *er = epsilon_r; *et = epsilon_t;
    scalar sr = 0, st = 0;
    for (int i = 0; i < 6; i++)
    {
        sr += A_(Ar_[i]) / (i + 1) * pow(phi, i + 1);
        st += A_(At_[i]) / (i + 1) * pow(phi, i + 1);
    }
    *scale = std::max(sr, st);
}

static void pr(const char* name, int ncol, const scalar* c, scalar scale, scalar v)
{
    std::printf("%s", name);
    for (int i = 0; i < ncol; i++) std::printf(" %.17g", c[i]);
    std::printf(" %.17g %.17g\n", scale, v);
}

int main()
{
    scalar sc, v;
    const scalar Bus[] = {0, 100, 1e3, 5e3, 1e4, 2e4, 5e4, 1e5};
    // ---- densification Empirical: cols Bu frac dc tau
    for (scalar frac : {0.9, 0.95, 0.97})
        for (scalar dc : {0.5, 1.5})
            for (scalar tau : {1000.0, 5000.0})
                for (scalar Bu : Bus)
                {
                    v = densEmpirical(Bu, frac, dc, tau, &sc);
                    const scalar c[] = {Bu, frac, dc, tau};
                    pr("DensEmpirical", 4, c, sc, v);
                }
    // ---- densification FRAPCON: cols T Bu frac resint Tsinter iters
    for (scalar T : {400.0, 900.0, 949.9, 950.0, 1000.0, 1049.9, 1050.0, 1200.0, 1500.0, 1900.0})
        for (scalar frac : {0.9, 0.95, 0.97})
            for (scalar rc : {0.0, 0.5, 1.5})
                for (scalar Ts : {1800.0, 1700.0})
                    for (scalar Bu : Bus)
                    {
                        int it = 0;
                        v = densFrapcon(T, Bu, frac, rc, Ts, &it, &sc);
                        const scalar c[] = {T, Bu, frac, rc, Ts, (scalar)it};
                        pr("DensFrapcon", 6, c, sc, v);
                    }
    // ---- relocation: cols Qlin Gap D Bu modified
    for (scalar D : {0.0082, 0.0093})
        for (scalar Gap : {1.7e-4, 2.5e-4})
            for (scalar Q : {1e8, 2e8, 3e8, 5e8, 8e8, 1.2e9})
                for (scalar Bu : {0.0, 40.0, 82.5, 82.51, 82.6, 91.0, 450.0, 1e3, 3e3, 4.4e3, 4.5e3, 6e3, 1e4, 3e4, 5e4, 1e5})
                    for (int mod : {1, 0})
                    {
                        const scalar Qlin = Q * constant::mathematical::pi * pow(D / 2.0, 2.0);
                        v = setRelocation(Q, Gap, D, Bu, mod == 1, &sc);
                        const scalar c[] = {Qlin, Gap, D, Bu, (scalar)mod};
                        pr("Reloc", 5, c, sc, v);
                    }
    // ---- swelling Constant / FrCrAl: cols s f_cm2
    for (scalar s : {1e-3, 4.5e-3})
        for (scalar f : {0.0, 1e20, 1e21, 1e22, 5e22})
        {
            v = swConstant(s, f, &sc);       { const scalar c[] = {s, f}; pr("SwConstant", 2, c, sc, v); }
        }
    for (scalar p1 : {4.5e-29, 1.0e-28})
        for (scalar f : {0.0, 1e20, 1e21, 1e22, 5e22})
        {
            v = swFrCrAl(p1, f, &sc);        { const scalar c[] = {p1, f}; pr("SwFrCrAl", 2, c, sc, v); }
        }
    // ---- swelling FRAPCON: cols Bu porosity
    for (scalar p : {0.0, 0.05, 0.12})
        for (scalar Bu : {0.0, 1e3, 5e3, 5.29e3, 5.3e3, 5.4e3, 1e4, 2e4, 5e4, 7e4, 7.05e4, 7.1e4, 8e4, 1e5})
        {
            const scalar rho = 10960.0 * (1.0 - p);
            v = swFrapcon(Bu, rho, &sc);     { const scalar c[] = {Bu, p}; pr("SwFrapcon", 2, c, sc, v); }
        }
    // ---- steel void swelling and Wright-Sham: cols T f_cm2
    for (scalar T : {500.0, 600.0, 700.0, 763.15, 800.0, 900.0, 1000.0})
        for (scalar f : {1e21, 5e21, 1e22, 5e22, 1e23})
        {
            v = swSteelVoid(T, f, 1.3e-5, -2.5, 490, 100, 3.9, &sc);   { const scalar c[] = {T, f}; pr("SwAim1", 2, c, sc, v); }
            v = swSteelVoid(T, f, 1.5e-3, -2.5, 450, 100, 2.75, &sc);  { const scalar c[] = {T, f}; pr("SwGen1515", 2, c, sc, v); }
            v = swWrightSham(T, f, &sc);                               { const scalar c[] = {T, f}; pr("SwWrightSham", 2, c, sc, v); }
        }
    // ---- BISON Zy growth: cols type f_cm2 N S
    for (int type = 0; type < 6; type++)
        for (scalar f : {1e20, 1e21, 5e21, 1e22, 1e23})
            for (int N : {1, 2, 4, 16, 64})
            {
                scalar xx, zz, S;
                swZy(type, f, N, &xx, &zz, &S);
                const scalar c[] = {(scalar)type, f, (scalar)N, S};
                pr("SwZy_xx", 4, c, 0.0, xx);
                pr("SwZy_zz", 4, c, 0.0, zz);
            }
    // ---- MATPRO UO2 stepping: cols T porosity Bu_final N
    for (scalar T : {600.0, 900.0, 1200.0, 1600.0, 2000.0})
        for (scalar p : {0.0, 0.05})
            for (scalar Bu : {1e4, 3e4, 6e4, 1e5})
                for (int N = 1; N <= 4096; N *= 4)
                {
                    scalar rb;
                    const scalar rho = 10960.0 * (1.0 - p);
                    v = swMatpro(T, rho, Bu, N, &rb);
                    const scalar c[] = {T, p, Bu, (scalar)N};
                    pr("SwMatpro", 4, c, rb, v);
                }
    // ---- FBR-MOX stepping: cols Bu_final N gapOpen
    for (scalar Bu : {2000.0, 9000.0, 9500.0, 20000.0, 60000.0})
        for (int N : {1, 16, 64, 256})
            for (int open : {1, 0})
            {
                scalar ab;
                v = swFbr(Bu, N, open == 1, &ab);
                const scalar c[] = {Bu, (scalar)N, (scalar)open};
                pr("SwFbr", 3, c, ab, v);
            }
    // ---- correlation PyC: cols f_cm2 fcf set
    const scalar Ar0[6] = {-1.24080, 0.00175, 0.08533, -0.01253, 0, 0};
    const scalar At0[6] = {-1.24080, 0.05307, 0.07620, -0.01245, 0, 0};
    const scalar Ar1[6] = {0.1, -0.2, 0.3, -0.4, 0.5, -0.6};
    const scalar At1[6] = {-0.6, 0.5, -0.4, 0.3, -0.2, 0.1};
    for (int set = 0; set < 2; set++)
        for (scalar fcf : {1.0, 1.3})
            for (scalar phi : {0.0, 0.1, 0.5, 1.0, 2.0, 2.5, 3.0})
            {
                const scalar f = phi * 1e21;   // 1e21 n/cm2 = 1e25 n/m2
                scalar er, et;
                swPyC(f, fcf, set == 0 ? Ar0 : Ar1, set == 0 ? At0 : At1, &er, &et, &sc);
                const scalar c[] = {f, fcf, (scalar)set};
                pr("SwPyC_r", 3, c, sc, er);
                pr("SwPyC_t", 3, c, sc, et);
            }
    return 0;
}
