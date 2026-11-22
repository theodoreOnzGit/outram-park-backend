// SPDX-License-Identifier: GPL-3.0-only
//
// Dump driver 2 for gh:#1393: OFFBEAT 80e84450a115b0c411e1bfa5d166379f6bf6c084
// heat-capacity, density, Young's-modulus, Poisson-ratio and emissivity
// kernels, transcribed into a standalone C++ program.  Same route and caveats
// as dump_conductivity.cpp (verbatim loop-body statements marked UPSTREAM,
// checked by check_verbatim.sh; no OpenFOAM library is run).
//
// The gas constant of the two MATPRO oxide heat capacities is
// `Foam::constant::physicoChemical::R.value()`.  In OpenFOAM-ESI v2506
// (/home/user/upstream-build/OpenFOAM-v2506) that is NA*k with
// NA = 6.0221417930e+23 (src/OpenFOAM/global/constants/fundamental/
// fundamentalConstants.C:121) and k = 1.38065e-23 (etc/controlDict:1116, the
// SICoeffs physicoChemical group), i.e. a 2006-vintage value, not CODATA 2018.
// Both are emitted: *_RFoam (the upstream-as-built value) and *_R2018 (the
// port's 8.314462618153240).
//
// Build: g++ -O2 -ffp-contract=off -o dump_properties dump_properties.cpp
#include <cmath>
#include <cstdio>
#include <algorithm>

typedef double scalar;
using std::pow; using std::exp; using std::min; using std::max;

static const scalar R_FOAM = 6.0221417930e+23 * 1.38065e-23;
static const scalar R_2018 = 8.31446261815324;

// ---- heatCapacityMatproUO2.C --------------------------------------------
static scalar hcMatproUO2(scalar Ti, scalar OM, scalar R)
{
    scalar K1 = 296.7, K2 = 2.43e-2, K3 = 8.745e7, phi = 535.285, ED = 1.577e5,
           par1 = 2.0, par2 = 2.0, par3 = 2.0;
        const scalar nominalValue =  K1*pow(phi, par1)*exp(phi/Ti)/pow(Ti*(exp(phi/Ti)-1), par2) // UPSTREAM
         + K2*Ti                                                     // UPSTREAM
         + (OM/2)*K3*ED/(R*pow(Ti, par3))*exp(-ED/(R*Ti));           // UPSTREAM
    return nominalValue;
}

// ---- heatCapacityMatproUPuO2.C ------------------------------------------
static scalar hcMatproUPuO2(scalar Ti, scalar OM_, scalar wPuO2_, scalar R_)
{
    scalar K1_ = 347.4, K2_ = 3.95e-4, K3_ = 3.86e7, theta_ = 571, ED_ = 1.967e5;
    scalar wUO2_ = 1 - wPuO2_;
    scalar hc_UO2;
    scalar hc_PuO2;
        hc_UO2 =  296.7*pow(535.285,2)*exp(535.285/Ti)               // UPSTREAM
                  /pow(Ti*(exp(535.285/Ti)-1),2)                     // UPSTREAM
                  + 2.43e-2*Ti                                       // UPSTREAM
                  + (OM_/2)*8.745e7*1.577e5/(R_*pow(Ti,2))*exp(-1.577e5/(R_*Ti)); // UPSTREAM
        hc_PuO2 =  K1_*pow(theta_,2)*exp(theta_/Ti)                  // UPSTREAM
                    /pow(Ti*(exp(theta_/Ti)-1),2)                    // UPSTREAM
                    + K2_*Ti                                         // UPSTREAM
                    + (OM_/2)*K3_*ED_/(R_*pow(Ti,2))*exp(-ED_/(R_*Ti)); // UPSTREAM
        const scalar nominalValue = wUO2_*hc_UO2 + wPuO2_*hc_PuO2;   // UPSTREAM
    return nominalValue;
}

// ---- heatCapacityFinkUPuO2.C --------------------------------------------
static scalar hcFink(scalar Ti)
{
    scalar C1_ = 322.49, C2_ = 1.4679e-2, C3_ = 0, theta_ = 587.41, Ea_ = 18531.7;
        const scalar nominalValue =
            C1_*pow(theta_/Ti,2.0)*exp(theta_/Ti)/pow(exp(theta_/Ti)-1.0,2.0) // UPSTREAM
          + 2*C2_*Ti                                                 // UPSTREAM
          + C3_*Ea_*exp(-Ea_/Ti)/pow(Ti,2.0);                        // UPSTREAM
    return nominalValue;
}

// ---- heatCapacityMatproZy.C ---------------------------------------------
static scalar hcMatproZy(scalar Ti)
{
        scalar nominalValue(0.0);
        scalar Tup(0);
        scalar Tlow(0);
        scalar CpUp(0);
        scalar CpLow(0);
        if (Ti <= 400) { Tlow = 300; Tup = 400; CpLow = 281; CpUp = 302; }
        else if (Ti <= 640) { Tlow = 400; Tup = 640; CpLow = 302; CpUp = 331; }
        else if (Ti <= 1090) { Tlow = 640; Tup = 1090; CpLow = 331; CpUp = 375; }
        else if (Ti <= 1093) { Tlow = 1090; Tup = 1093; CpLow = 375; CpUp = 502; }
        else if (Ti <= 1113) { Tlow = 1090; Tup = 1113; CpLow = 502; CpUp = 590; }
        else if (Ti <= 1133) { Tlow = 1113; Tup = 1133; CpLow = 590; CpUp = 615; }
        else if (Ti <= 1153) { Tlow = 1133; Tup = 1153; CpLow = 615; CpUp = 719; }
        else if (Ti <= 1173) { Tlow = 1153; Tup = 1173; CpLow = 719; CpUp = 816; }
        else if (Ti <= 1193) { Tlow = 1173; Tup = 1193; CpLow = 816; CpUp = 770; }
        else if (Ti <= 1213) { Tlow = 1193; Tup = 1213; CpLow = 770; CpUp = 619; }
        else if (Ti <= 1233) { Tlow = 1213; Tup = 1233; CpLow = 619; CpUp = 469; }
        else if (Ti <= 1248) { Tlow = 1233; Tup = 1248; CpLow = 469; CpUp = 356; }
        else { Tlow = 1248; Tup = 2099; CpLow = 356; CpUp = 356; }
        nominalValue = CpLow + (CpUp - CpLow) * (Ti - Tlow) / (Tup - Tlow); // UPSTREAM
    return nominalValue;
}

// ---- heatCapacityIAEAZy.C -----------------------------------------------
static scalar hcIaeaZy(scalar Ti)
{
    scalar par1 = 255.66, par2 = 0.1024, par3 = 597.1, par4 = 0.4088,
           par5 = 1.565E-4, par6 = 1058.4, par7 = 1213.8, par8 = 2.0, par9 = 719.61;
        scalar Cp1 = par1 + par2*Ti;                                 // UPSTREAM
        scalar Cp2 = par3 - par4*Ti + par5*Ti*Ti;                    // UPSTREAM
        scalar f = par6*exp(-pow(Ti - par7,par8)/par9);              // UPSTREAM
        scalar nominalValue(0.0);
        if (Ti < 1100) { nominalValue = Cp1; }
        else if (Ti < 1213.8) { nominalValue = Cp1 + f; }
        else if (Ti < 1320) { nominalValue = Cp2 + f; }
        else { nominalValue = Cp2; }
    return nominalValue;
}

// ---- heatCapacityBanerjee1515Ti.C / Molybdenum / SneadSiC ---------------
static scalar hcBanerjee(scalar Ti)
{
    scalar par1 = 431, par2 = 0.177, par3 = 8.72e-5;
        const scalar nominalValue = par1 + par2 * Ti + par3 * pow(Ti,-2); // UPSTREAM
    return nominalValue;
}
static scalar hcMolybdenum(scalar Ti)
{
    scalar par1 = 9.74E-06, par2 = 5.37E-2, par3 = 235;
        const scalar nominalValue = par1*Ti*Ti + par2*Ti + par3;     // UPSTREAM
    return nominalValue;
}
static scalar hcSnead(scalar Ti)
{
    // constructor initialiser values (the dictionary default for par4 is -3.19446e7)
    scalar par1 = 925.65, par2 = 0.3772, par3 = -7.9259e-5, par4 = -3.1946e7;
        const scalar nominalValue = par1 + par2 * Ti + par3 * pow(Ti,2) + par4 * pow(Ti,-2); // UPSTREAM
    return nominalValue;
}

// ---- densityIAEAZy.C / densitySchumann1515Ti.C --------------------------
static scalar rhoIaeaZy(scalar Ti)
{
    scalar par1 = 6595.2, par2 = 0.1477, par3 = 6690.0, par4 = 0.1855;
        scalar rho1 = par1 - par2*Ti;                                // UPSTREAM
        scalar rho2 = par3 - par4*Ti;                                // UPSTREAM
        scalar nominalValue(0.0);
        if (Ti < 1083) { nominalValue = rho1; }
        else if (Ti < 1144)
        {
            nominalValue = rho1 + (rho2-rho1)*(Ti-1083)/(1144-1083); // UPSTREAM
        }
        else { nominalValue = rho2; }
    return nominalValue;
}
static scalar rhoSchumann(scalar T)
{
    scalar rho0_ = 7900.0;
    scalar linExpCoeff;
        const scalar Ti = T - 273.15;
        linExpCoeff = -3.101e-4 + 1.545e-5*Ti + 2.75e-9*pow(Ti,2);  // UPSTREAM
        const scalar nominalValue =
        rho0_ * pow(1/(1+linExpCoeff),3);                            // UPSTREAM
    return nominalValue;
}

// ---- Young's modulus -----------------------------------------------------
static scalar eMatproUO2(scalar Ti, scalar densityFrac_)
{
    scalar par1 = 2.334e11, par2 = 2.752, par3 = 1.0915e-4;
        const scalar nominalValue = par1*(1 - par2*(1-densityFrac_))*(1 - par3*Ti); // UPSTREAM
    return nominalValue;
}
static scalar eMatproUPuO2(scalar Ti, scalar densityFrac_, scalar OM, scalar conc_at_Pu)
{
    scalar par1 = 2.334e11, par2 = 2.752, par3 = 1.0915e-4;
    scalar x = 2-OM;
    scalar B = (x < 0) ? 1.75 : 1.35;
        const scalar k_Pu = 1.13;                                    // UPSTREAM
        const scalar c_w_Pu = conc_at_Pu / k_Pu;                     // UPSTREAM
        const scalar ES = par1*(1 - par2*(1-densityFrac_))*(1 - par3*Ti); // UPSTREAM
        const scalar nominalValue = ES*exp(-B*x)*(1+0.05*c_w_Pu);    // UPSTREAM
    return nominalValue;
}
static scalar eSckCen(scalar Ti, scalar porosity, scalar y_, scalar conc_at_Pu)
{
    scalar E1_ = 219.12, E2_ = -0.0154, E3_ = -9.0e-6, EUO2_ = 218.74, EPuO2_ = 249.45;
    scalar dEdy_ = 0;
        const scalar k_Pu = 1.13;                                    // UPSTREAM
        const scalar c_w_Pu = conc_at_Pu / k_Pu;                     // UPSTREAM
        const scalar Emox0 = (1-c_w_Pu)*EUO2_ + c_w_Pu*EPuO2_;       // UPSTREAM
        scalar E(0);
        if( y_>= 0 and y_ <= 0.037 )
        {
            dEdy_ = -586;                                            // UPSTREAM
            E = Emox0 + y_ * dEdy_;                                  // UPSTREAM
        }
        else if (y_>0.037)
        {
            dEdy_ = -126.59;                                         // UPSTREAM
            scalar Elim = Emox0 -  0.037 * 586;                      // UPSTREAM
            E = Elim + (y_ - 0.037) * dEdy_;                         // UPSTREAM
        }
        else
        {
            E = Emox0;                                               // UPSTREAM
        }
        const scalar fT   = E1_ + E2_*Ti     + E3_*pow(Ti,2);        // UPSTREAM
        const scalar f273 = E1_ + E2_*273.00 + E3_*pow(273.00,2);    // UPSTREAM
        scalar nominalValue = E*fT/f273*1e9;                         // UPSTREAM
        scalar ppp = min(porosity, 0.3);
        nominalValue *= pow((1 - ppp),2.0)/ (1.0 + ppp*1.1);         // UPSTREAM
    return nominalValue;
}
static scalar eHofman(scalar T)
{
    scalar par1 = 2.01e5, par2 = 79.29;
        const scalar Ti = T - 273.15;
        const scalar nominalValue = (par1-par2*Ti)*1e6;              // UPSTREAM
    return nominalValue;
}
static scalar eMolybdenum(scalar T)
{
        const scalar nominalValue = 3.349E+11 - 5.101E+07*T;         // UPSTREAM
    return nominalValue;
}
static scalar eTobbe(scalar T)
{
    scalar par1 = 202.7, par2 = 81.67e-3;
        const scalar Ti = T - 273.15;
        const scalar nominalValue = (par1-par2*Ti)*1e9;              // UPSTREAM
    return nominalValue;
}
static scalar eWatrous(scalar Ti)
{
    scalar par1 = 9.944e-08, par2 = 1.178e-4, par3 = 1.033e-1, par4 = 220.9;
    scalar nominalValue(0.0);
            scalar K1 = -par1*(Ti-273.15)*(Ti-273.15)*(Ti-273.15);   // UPSTREAM
            scalar K2 = par2*(Ti-273.15)*(Ti-273.15);                // UPSTREAM
            scalar K3 = -par3*(Ti-273.15) + par4;                    // UPSTREAM
            nominalValue = (K1 + K2 + K3)*1e9;                       // UPSTREAM
    return nominalValue;
}

// ---- PoissonRatioTobbe1515Ti.C, emissivityRelapUO2.C ---------------------
static scalar nuTobbe(scalar T)
{
    scalar par1 = 0.277, par2 = 6e-5;
        const scalar Ti = T - 273.15;
        const scalar nominalValue = par1 + par2*Ti;                  // UPSTREAM
    return nominalValue;
}
static scalar emRelap(scalar Ti)
{
    scalar par1 = 0.7856, par2 = 1.5263e-5;
        const scalar nominalValue =  par1 + par2*Ti;                 // UPSTREAM
    return nominalValue;
}

// columns: T p OM w
static void emit(const char* name, scalar T, scalar p, scalar OM, scalar w, scalar v)
{
    std::printf("%s %.17g %.17g %.17g %.17g %.17g\n", name, T, p, OM, w, v);
}

int main()
{
    const scalar Ts[] = {273, 290, 300, 333.33, 400, 500, 600, 640, 700, 800, 900,
        1000, 1085.5, 1090, 1100, 1120, 1200, 1234.5678, 1250, 1273, 1300, 1500, 1700,
        1800, 2000, 2200, 2400, 2718.2818, 3000};
    const scalar Ps[] = {0, 0.05, 0.12, 0.2};
    const scalar OMs[] = {1.94, 1.97, 2.0, 2.03};
    const scalar Ws[] = {0.0, 0.05, 0.3};

    for (scalar T : Ts)
    {
        for (scalar OM : OMs)
        {
            emit("HcMatproUo2_RFoam", T, 0, OM, 0, hcMatproUO2(T, OM, R_FOAM));
            emit("HcMatproUo2_R2018", T, 0, OM, 0, hcMatproUO2(T, OM, R_2018));
            for (scalar w : Ws)
            {
                emit("HcMatproMox_RFoam", T, 0, OM, w, hcMatproUPuO2(T, OM, w, R_FOAM));
                emit("HcMatproMox_R2018", T, 0, OM, w, hcMatproUPuO2(T, OM, w, R_2018));
            }
        }
        emit("HcFinkMox", T, 0, 2.0, 0, hcFink(T));
        emit("HcMatproZircaloy", T, 0, 2.0, 0, hcMatproZy(T));
        emit("HcIaeaZircaloy", T, 0, 2.0, 0, hcIaeaZy(T));
        emit("HcBanerjee1515Ti", T, 0, 2.0, 0, hcBanerjee(T));
        emit("HcMolybdenum", T, 0, 2.0, 0, hcMolybdenum(T));
        emit("HcSneadSiC", T, 0, 2.0, 0, hcSnead(T));
        emit("RhoIaeaZy", T, 0, 2.0, 0, rhoIaeaZy(T));
        emit("RhoSchumann1515Ti", T, 0, 2.0, 0, rhoSchumann(T));
        emit("EHofmanD9", T, 0, 2.0, 0, eHofman(T));
        emit("EBisonMolybdenum", T, 0, 2.0, 0, eMolybdenum(T));
        emit("ETobbe1515Ti", T, 0, 2.0, 0, eTobbe(T));
        emit("EWatrousHastelloyN", T, 0, 2.0, 0, eWatrous(T));
        emit("NuTobbe1515Ti", T, 0, 2.0, 0, nuTobbe(T));
        emit("EmRelapUo2", T, 0, 2.0, 0, emRelap(T));
        for (scalar p : Ps)
        {
            emit("EMatproUo2", T, p, 2.0, 0, eMatproUO2(T, 1-p));
            for (scalar OM : OMs)
            {
                for (scalar w : Ws)
                {
                    emit("EMatproMox", T, p, OM, w, eMatproUPuO2(T, 1-p, OM, w));
                    emit("ESckCenMox", T, p, OM, w, eSckCen(T, p, 2-OM, w));
                }
            }
        }
    }
    return 0;
}
