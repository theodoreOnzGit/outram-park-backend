// SPDX-License-Identifier: GPL-3.0-only
//
// Dump driver for gh:#1393: OFFBEAT 80e84450a115b0c411e1bfa5d166379f6bf6c084
// thermal-conductivity kernels, transcribed into a standalone C++ program.
//
// ROUTE (recorded honestly): this is NOT a run of the compiled OFFBEAT
// library.  Each function below copies, character for character, the
// arithmetic statements of the `forAll(addr, i)` body of the named upstream
// `correct(scalarField&, const scalarField& T, const labelList&)`.  The
// OpenFOAM context (mesh registry look-ups, dictionary defaults) is replaced
// by explicit arguments; the dictionary defaults are copied from the
// constructors.  `check_verbatim.sh` checks that every line marked UPSTREAM
// occurs in the upstream file, which is the evidence the copy is verbatim.
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat), GPL-3.0.
//
// Build: g++ -O2 -ffp-contract=off -o dump_conductivity dump_conductivity.cpp
#include <cmath>
#include <cstdio>
#include <algorithm>

typedef double scalar;
using std::pow; using std::exp; using std::tanh; using std::sqrt;
using std::min; using std::max;

// ---- conductivityMatproUO2.C -------------------------------------------
// Bu in MWd/tUO2 (OFFBEAT burnup field), densityFrac from porosity, swelling
// fields Intragranular/Intergranular, epsilonDensification linear.
static scalar matproUO2(scalar Ti, scalar Bu, scalar p, scalar GdContent_,
                        scalar intra, scalar inter, scalar epsD)
{
    scalar par1 = 0.0452, par2 = 0.000246, par3 = 0.00187, par4 = 1.1599,
           par5 = 0.9, par6 = 0.04, par7 = 0.038, par8 = 0.28, par9 = 396.0,
           par10 = 6380.0, par11 = 3.5e9, par12 = 2.0, par13 = 16360.0;
    scalar volumeCorr = 1;
    volumeCorr += 3*intra/3;
    volumeCorr += 3*inter/3;
    volumeCorr += 3*epsD;
    scalar densityFrac = max(1-p, 0.05);
        const scalar Bui = Bu/1000/0.881;                            // UPSTREAM
        double term0 = par1 ;                                        // UPSTREAM
        double term1 = par2*Ti;                                      // UPSTREAM
        double term2 = par3*Bui;                                     // UPSTREAM
        double term3 = par4*GdContent_;                              // UPSTREAM
        double term4 = (1-par5*exp(-par6*Bui))*par7*pow(Bui,par8);   // UPSTREAM
        double term5 = 1/(1+par9*exp(-par10/Ti));                    // UPSTREAM
        double term6 = (par11/pow(Ti,par12))*exp(-par13/Ti);         // UPSTREAM
        double k95 = 1/(term0 + term1 + term2 + term3 + term4*term5) + term6; // UPSTREAM
        const scalar denFracNew = densityFrac/volumeCorr;
        const scalar nominalValue = k95*1.0789*denFracNew/(1+0.5*(1-denFracNew)); // UPSTREAM
    return nominalValue;
}

// ---- conductivityNfirUO2.C ----------------------------------------------
static scalar nfirUO2(scalar T, scalar Bu, scalar densityFrac_, scalar GdContent_,
                      scalar intra, scalar inter, scalar epsD)
{
    scalar volumeCorr = 1;
    volumeCorr += 3*intra/3;
    volumeCorr += 3*inter/3;
    volumeCorr += 3*epsD;
        const scalar Bui = Bu/1000/0.881;                            // UPSTREAM
        const scalar Ti = T - 273.15;
        scalar term0(0);
        if(GdContent_ > 0)
        {
            scalar Gd = GdContent_;
            scalar Gdir = 0.1197*Gd + pow(tanh(Gd), 0.1)             // UPSTREAM
            + 1.214167e-2*Gd + 5.40625e-4*Gd*Gd - 5.182292e-5*Gd*Gd*Gd; // UPSTREAM
            scalar Gdfac = (0.027273*Gd + 0.65227273) -              // UPSTREAM
            (2.25e-2*Gd*tanh(0.00001*Bui));                          // UPSTREAM
            term0 = Gdir*Gdfac;                                      // UPSTREAM
        }
        else
        {
            term0 = 9.592e-2;                                        // UPSTREAM
        }
        scalar RF = 0.5*(1 + tanh((Ti - 900)/150));                  // UPSTREAM
        scalar kPhononStart = 1/(term0 + 6.14e-3*Bui - 1.4e-5*Bui*Bui // UPSTREAM
            + (2.5e-4 - 1.81e-6*Bui)*Ti);                            // UPSTREAM
        scalar kPhononEnd = 1/(term0 + 2.6e-3*Bui + (2.5e-4 - 2.7e-7*Bui)*Ti); // UPSTREAM
        scalar kElectronic = 1.32e-2*exp(1.88e-3*Ti);                // UPSTREAM
        double k95 = (1 - RF)*kPhononStart + RF*kPhononEnd + kElectronic; // UPSTREAM
        const scalar denFracNew = densityFrac_/volumeCorr;           // UPSTREAM
        const scalar nominalValue = k95*                             // UPSTREAM
        (1 - (2.58 - 5.8e-4*Ti)*(1 - denFracNew))/                   // UPSTREAM
        (1 - 0.05*(2.58 - 5.8e-4*Ti));                               // UPSTREAM
    return nominalValue;
}

// ---- conductivityUO2IFA601.C --------------------------------------------
static scalar ifa601(scalar Ti, scalar Bu)
{
    scalar par1 = 40, par2 = 5, par3 = 0.24, par4 = 0.457, par5 = 0.00433,
           par6 = 0.11, par7 = 0.0000132, par8 = 0.00188;
        const scalar Bui = Bu/1000;                                  // UPSTREAM
        double term1 = par1 + par2*Bui;                              // UPSTREAM
        double term2 = par3;                                         // UPSTREAM
        if(Bui > 50)
        {
            if(Bui <= 80)
            {
                term2 = par4 - par5*Bui;                             // UPSTREAM
            }
            else
            {
                term2 = par6;                                        // UPSTREAM
            }
        }
        const scalar nominalValue = 1/(term1 + term2*Ti) + par7*exp(par8*Ti); // UPSTREAM
    return nominalValue*1000;
}

// ---- conductivityUPuO2Brancheria.C --------------------------------------
static scalar brancheria(scalar Ti, scalar pi)
{
    scalar perturb = 1;
        scalar D1 = (pi <= 0.05)                                     // UPSTREAM
        ? 1 - 1.5*pi                                                 // UPSTREAM
        : 1 - min(pi, 0.15) - 10 * pow(min(pi, 0.15), 2.0);          // UPSTREAM
        scalar nominalValue =                                        // UPSTREAM
            1e2 * (D1 * (pow(2.88+0.0252*Ti, -1) + 5.83e-13*pow(Ti,3.0))); // UPSTREAM
    return nominalValue*perturb;
}

// ---- conductivityUPuO2Kato.C --------------------------------------------
// conc_at_Am / conc_at_Np passed directly as atom fractions.
static scalar kato(scalar Ti, scalar OM, scalar conc_at_Am, scalar conc_at_Np, scalar Pi)
{
        const scalar x = 2 - OM;                                     // UPSTREAM
        const scalar k_Am = 1.12;                                    // UPSTREAM
        const scalar k_Np = 1.14;                                    // UPSTREAM
        const scalar c_w_Am = conc_at_Am / k_Am;                     // UPSTREAM
        const scalar c_w_Np = conc_at_Np / k_Np;                     // UPSTREAM
        const scalar den = 1.595e-2 + 2.713*x + 3.583e-1*c_w_Am      // UPSTREAM
                         + 6.317e-2 *c_w_Np + (-2.625*x + 2.493)*1e-4*Ti; // UPSTREAM
        const scalar k = (1/(den)+1.541e11/pow(Ti,2.5)*exp(-1.522e4/Ti)); // UPSTREAM
        const scalar kappaHe(0.69);                                  // UPSTREAM
        const scalar nominalValue = k * (kappaHe+2*k-2*Pi*(k-kappaHe)) // UPSTREAM
                                      / (kappaHe+2*k + Pi*(k-kappaHe)); // UPSTREAM
    return nominalValue;
}

// ---- conductivityUPuO2LanningBeyer.C ------------------------------------
static scalar lanningBeyer(scalar Ti, scalar Bu, scalar OM, scalar porosity_)
{
        const scalar Bui = Bu*1e-3;                                  // UPSTREAM
        const scalar x = 2 - OM;                                     // UPSTREAM
        const scalar A = 0.035 + 2.8*x;                              // UPSTREAM
        const scalar B = (2.86-7.15*x)*1e-4;                         // UPSTREAM
        const scalar f = 1.87e-3*Bui;                                // UPSTREAM
        const scalar g = 0.038*pow(Bui,0.28);                        // UPSTREAM
        const scalar h = pow(1+396*exp(-6380/Ti),-1);                // UPSTREAM
        const scalar C = 1.5e9;                                      // UPSTREAM
        const scalar D = 13520;                                      // UPSTREAM
        const scalar k95 =                                           // UPSTREAM
        pow((A+B*Ti+f+(1-0.9*exp(-0.04*Bui))*g*h),-1.0)+C*pow(Ti,-2.0)*exp(-D/Ti); // UPSTREAM
        const scalar nominalValue = k95 * 1.0789 * (1-porosity_) / (1+0.5*porosity_); // UPSTREAM
    return nominalValue;
}

// ---- conductivityUPuO2MagniMAMOX.C --------------------------------------
static scalar magni(scalar Ti, scalar Bu, scalar por, scalar OM, scalar conc_at_Pu,
                    scalar conc_at_Am, scalar conc_at_Np)
{
    scalar kappaInf_ = 1.755, phi_ = 128.75e3, A0_ = 0.01926, Ax_ = 1.06e-6,
           APu_ = 2.63e-8, AAm_ = 0.596, ANp_ = 2.22e-14, B0_ = 2.39e-4,
           BPu_ = 1.37e-13, BAm_ = 5.47e-4, BNp_ = 2.48e-14, D_ = 5.27e9,
           E_ = 17109.5;
        const scalar Bui = Bu;                                       // UPSTREAM
        const scalar porI = min(por, 0.95);
        const scalar k_Pu = 1.13;                                    // UPSTREAM
        const scalar k_Am = 1.12;                                    // UPSTREAM
        const scalar k_Np = 1.14;                                    // UPSTREAM
        const scalar c_w_Pu = conc_at_Pu / k_Pu;                     // UPSTREAM
        const scalar c_w_Am = conc_at_Am / k_Am;                     // UPSTREAM
        const scalar c_w_Np = conc_at_Np / k_Np;                     // UPSTREAM
        const scalar x = 2 - OM;                                     // UPSTREAM
        double term1 =                                               // UPSTREAM
            pow                                                      // UPSTREAM
            (                                                        // UPSTREAM
                A0_ + Ax_ * x + APu_ * c_w_Pu + AAm_ * c_w_Am + ANp_ * c_w_Np // UPSTREAM
                + ( B0_ + BPu_ * c_w_Pu + BAm_ * c_w_Am + BNp_ * c_w_Np ) * Ti // UPSTREAM
                ,                                                    // UPSTREAM
                -1.0                                                 // UPSTREAM
            );                                                       // UPSTREAM
        double term2 = D_ / pow(Ti , 2.0) * exp(-E_ / Ti);           // UPSTREAM
        double term3 = pow(1-porI , 2.5);                            // UPSTREAM
        double k0    = (term1 + term2) * term3;                      // UPSTREAM
        const scalar nominalValue = kappaInf_ + (k0 - kappaInf_)*exp(-Bui/phi_); // UPSTREAM
    return nominalValue;
}

// ---- conductivityUPuO2Philipponneau.C -----------------------------------
static scalar philipponneau(scalar Ti, scalar Bu, scalar por, scalar OM)
{
    scalar B_ = 2.885e-4, C_ = 76.38e-12;
    const scalar BuFIMA = Bu * 1e-3 / 9.5 / 100;                     // UPSTREAM
        const scalar tau = BuFIMA;
        const scalar& poroI = min(por, 0.95);                        // UPSTREAM
        const scalar x = 2-OM;                                       // UPSTREAM
        const scalar A = 1.528 * sqrt(x+0.00931) - 0.1055 + 0.44*tau; // UPSTREAM
        const scalar alpha = 1/0.864 * (1-poroI)/(1 + 2*poroI);      // UPSTREAM
        const scalar nominalValue = ( 1/(A + B_*Ti) + C_*pow(Ti,3.0) ) * alpha; // UPSTREAM
    return nominalValue;
}

// ---- conductivityRelapZy.C ----------------------------------------------
static scalar relapZy(scalar Ti)
{
    scalar par1 = 7.51, par2 = 2.09e-2, par3 = 1.45e-5, par4 = 7.67e-9;
        const scalar nominalValue =  par1 + par2*Ti - par3*Ti*Ti + par4*Ti*Ti*Ti; // UPSTREAM
    return nominalValue;
}

// ---- conductivityMolybdenum.C -------------------------------------------
static scalar molybdenum(scalar Ti)
{
    scalar par1 = 9.128E-06, par2 = 4.945E-2, par3 = 152;
        const scalar nominalValue = par1*Ti*Ti - par2*Ti + par3;     // UPSTREAM
    return nominalValue;
}

// ---- conductivitySwindemanHastelloyN.C ----------------------------------
static scalar swindeman(scalar Ti)
{
    scalar par1 = 8.431, par2 = 0.0205;
        const scalar nominalValue =  par1 + par2*(Ti-273.15) ;       // UPSTREAM
    return nominalValue;
}

// ---- conductivityTobbe1515Ti.C ------------------------------------------
static scalar tobbe(scalar T)
{
    scalar par1 = 13.95, par2 = 1.163e-2;
        const scalar Ti = T-273.15;
        const scalar nominalValue =  par1 + par2*Ti;                 // UPSTREAM
    return nominalValue;
}

// ---- conductivityPARFUMEBuffer.C ----------------------------------------
static scalar parfumeBuffer(scalar rhoI)
{
    scalar rho_init_ = 1000.0, rho_theo_ = 2250.0, k_init_ = 0.5, k_theo_ = 4.0;
        const scalar nominalValue =  k_init_*k_theo_*rho_theo_*(rho_theo_-rho_init_) / // UPSTREAM
                                     (k_theo_*rho_theo_*(rho_theo_-rhoI)+k_init_*rhoI*(rhoI-rho_init_)); // UPSTREAM
    return nominalValue;
}

// ---- conductivityPARFUMESiC.C -------------------------------------------
static scalar parfumeSiC(scalar Ti)
{
    scalar par1 = 17885.0, par2 = 2.0;
        scalar nominalValue(0.0);
        nominalValue = par1 / Ti + par2;                             // UPSTREAM
    return nominalValue;
}

static void emit(const char* name, scalar T, scalar Bu, scalar por, scalar OM,
                 scalar Gd, scalar pu, scalar am, scalar sw, scalar epsd, scalar v)
{
    std::printf("%s %.17g %.17g %.17g %.17g %.17g %.17g %.17g %.17g %.17g %.17g\n",
                name, T, Bu, por, OM, Gd, pu, am, sw, epsd, v);
}

int main()
{
    const scalar Ts[] = {290, 300, 333.33, 400, 500, 600, 700, 800, 900, 1000, 1100,
        1234.5678, 1364, 1500, 1600, 1700, 1800, 1834.5, 2000, 2200, 2400, 2600,
        2718.2818, 2800, 3000, 3113};
    const scalar Bus[] = {0, 5e3, 2.5e4, 5.5e4, 9e4, 1.2e5};
    const scalar Ps[] = {0, 0.05, 0.12, 0.2};
    const scalar OMs[] = {1.94, 1.97, 2.0};
    const scalar Gds[] = {0, 0.04, 0.08};
    const scalar Pus[] = {0.05, 0.2};
    const scalar Ams[] = {0.0, 0.002};

    for (scalar T : Ts)
    {
        for (scalar Bu : Bus)
        {
            for (scalar p : Ps)
            {
                for (scalar Gd : Gds)
                {
                    emit("MatproUo2", T, Bu, p, 2.0, Gd, 0, 0, 0, 0, matproUO2(T, Bu, p, Gd, 0, 0, 0));
                    emit("NfirUo2", T, Bu, p, 2.0, Gd, 0, 0, 0, 0, nfirUO2(T, Bu, 1-p, Gd, 0, 0, 0));
                }
                for (scalar OM : OMs)
                {
                    emit("LanningBeyerMox", T, Bu, p, OM, 0, 0, 0, 0, 0, lanningBeyer(T, Bu, OM, p));
                    emit("PhilipponneauMox", T, Bu, p, OM, 0, 0, 0, 0, 0, philipponneau(T, Bu, p, OM));
                    for (scalar pu : Pus)
                    {
                        for (scalar am : Ams)
                        {
                            emit("MagniMamox", T, Bu, p, OM, 0, pu, am, 0, 0, magni(T, Bu, p, OM, pu, am, 0.0));
                        }
                    }
                }
            }
            emit("Ifa601Uo2", T, Bu, 0, 2.0, 0, 0, 0, 0, 0, ifa601(T, Bu));
        }
        for (scalar p : Ps)
        {
            emit("BrancheriaMox", T, 0, p, 2.0, 0, 0, 0, 0, 0, brancheria(T, p));
            for (scalar OM : OMs)
            {
                for (scalar am : Ams)
                {
                    emit("KatoMox", T, 0, p, OM, 0, 0, am, 0, 0, kato(T, OM, am, 0.0, p));
                }
            }
            emit("ParfumeBuffer", T, 0, p, 2.0, 0, 0, 0, 0, 0, parfumeBuffer(2250.0*(1-p)));
        }
        emit("RelapZircaloy", T, 0, 0, 2.0, 0, 0, 0, 0, 0, relapZy(T));
        emit("Molybdenum", T, 0, 0, 2.0, 0, 0, 0, 0, 0, molybdenum(T));
        emit("SwindemanHastelloyN", T, 0, 0, 2.0, 0, 0, 0, 0, 0, swindeman(T));
        emit("Tobbe1515Ti", T, 0, 0, 2.0, 0, 0, 0, 0, 0, tobbe(T));
        emit("ParfumeSiC", T, 0, 0, 2.0, 0, 0, 0, 0, 0, parfumeSiC(T));
    }

    // Volume-correction rows (swelling field and linear densification strain).
    const scalar vT[] = {600.0, 1200.0, 2000.0};
    const scalar vBu[] = {0.0, 2.5e4, 7e4};
    const scalar vSw[] = {0.0, 0.013, 0.05};
    const scalar vEps[] = {0.0, -0.004, -0.01};
    for (scalar T : vT)
    {
        for (scalar Bu : vBu)
        {
            for (scalar sw : vSw)
            {
                for (scalar epsd : vEps)
                {
                    emit("MatproUo2_vol", T, Bu, 0.05, 2.0, 0, 0, 0, sw, epsd, matproUO2(T, Bu, 0.05, 0.0, sw, 0, epsd));
                    emit("NfirUo2_vol", T, Bu, 0.05, 2.0, 0, 0, 0, sw, epsd, nfirUO2(T, Bu, 0.95, 0.0, sw, 0, epsd));
                }
            }
        }
    }
    return 0;
}
