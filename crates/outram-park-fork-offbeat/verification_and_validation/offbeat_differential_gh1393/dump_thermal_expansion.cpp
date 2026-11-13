// SPDX-License-Identifier: GPL-3.0-only
//
// Dump driver 3 for gh:#1393 round 2: OFFBEAT 80e84450a115b0c411e1bfa5d166379f6bf6c084
// thermal-expansion kernels (`thermalExpansion*.C`, 14 classes), transcribed
// into a standalone C++ program.  Same route and caveats as
// dump_conductivity.cpp: statements marked UPSTREAM are copied verbatim from
// the `correct()` loop bodies (checked by check_verbatim.sh); no OpenFOAM
// library is run.  Shim: `symmTensor` is a scalar and `I` is 1.0 (every
// upstream tensor here is a scalar multiple of the identity), so the
// `(...)*I` lines are verbatim.  The "numerical dead zone" `if (mag(xx)<1e-7
// && mag(yy)<1e-7 && mag(zz)<1e-7)` is replaced by the scalar test on the
// shared diagonal value (not marked UPSTREAM).
//
// Output columns: name T Tref a b c d scale value
//   scale = sum of |additive terms| of both operands (the gate scale, see
//   PREDICTIONS_round2.md section A).
//
// Build: g++ -O2 -ffp-contract=off -o dump_thermal_expansion dump_thermal_expansion.cpp
#include <cmath>
#include <cstdio>
#include <algorithm>

typedef double scalar;
typedef double symmTensor;
using std::pow; using std::exp; using std::min; using std::max;
static const scalar I = 1.0;
static scalar mag(scalar x) { return std::fabs(x); }

// dead zone with mag() (every class except PARFUME buffer / PyC)
static scalar dz(scalar v) { return (mag(v) < 1e-7) ? 0.0 : v; }
// dead zone WITHOUT mag() (PARFUME buffer / PyC as written upstream)
static scalar dz_unsigned(scalar v) { return (v < 1e-7) ? 0.0 : v; }

static scalar A(scalar x) { return std::fabs(x); }

// ---- thermalExpansionConstant.C -----------------------------------------
static void constant(scalar Ti, scalar Tref, scalar alpha, scalar* val, scalar* sc)
{
    const scalar T0 = Ti;
    scalar Tref_value = Tref, alpha_value = alpha;
        symmTensor nominalValue(I*(alpha_value*(T0-Tref_value)));   // UPSTREAM (alpha_.value() etc. renamed)
    *val = dz(nominalValue);
    *sc = A(alpha * Ti) + A(alpha * Tref);
}

// ---- thermalExpansionRelapUO2.C -----------------------------------------
static void relap(scalar Ti, scalar Tref, scalar* val, scalar* sc)
{
    scalar K1 = 9.8e-6, K2 = 2.61e-3, K3 = 3.16e-1, ED = 1.32e-19, k = 1.38e-23;
    const symmTensor referenceValue = (K1*Tref - K2 + K3*exp(-ED/k/Tref))*I; // UPSTREAM
        symmTensor nominalValue =                                    // UPSTREAM
        (K1*Ti - K2 + K3*exp(-ED/k/Ti))*I - referenceValue;          // UPSTREAM
    *val = dz(nominalValue);
    *sc = A(K1*Ti) + K2 + A(K3*exp(-ED/k/Ti)) + A(K1*Tref) + K2 + A(K3*exp(-ED/k/Tref));
}

// ---- thermalExpansionMatproUPuO2.C (N_Am = N_Np = 0) ---------------------
static void matproUPuO2(scalar T, scalar N_U, scalar N_Pu, scalar* val, scalar* sc, scalar* concOut)
{
    scalar par1_ = -3.9735e-4, par2_ = 8.4955e-6, par3_ = 2.15130e-9, par4_ = 3.7143e-16,
           par5_ = -4.972e-4, par6_ = 7.107e-6, par7_ = 2.581e-9, par8_ = 1.140e-13;
        const scalar Ti = T - 273.15;                                // UPSTREAM
        const scalar N_Am = 0.0;
        const scalar N_Np = 0.0;
        const scalar conc_at_Pu = N_Pu / (N_Pu + N_Am + N_Np + N_U); // UPSTREAM
        const scalar k_Pu = 1.13;                                    // UPSTREAM
        const scalar c_w_Pu = conc_at_Pu / k_Pu;                     // UPSTREAM
        scalar dL_L_PuO2 =                                           // UPSTREAM
            (par1_ + par2_*Ti + par3_*pow(Ti,2) + par4_*pow(Ti,3));  // UPSTREAM
        scalar dL_L_UO2 =                                            // UPSTREAM
            (par5_ + par6_*Ti + par7_*pow(Ti,2) + par8_*pow(Ti,3));  // UPSTREAM
        scalar dL_L = c_w_Pu * dL_L_PuO2 + ( 1-c_w_Pu) * dL_L_UO2;   // UPSTREAM
        symmTensor nominalValue = dL_L * I;                          // UPSTREAM
    *val = dz(nominalValue);
    const scalar sPu = A(par1_) + A(par2_*Ti) + A(par3_*Ti*Ti) + A(par4_*Ti*Ti*Ti);
    const scalar sU = A(par5_) + A(par6_*Ti) + A(par7_*Ti*Ti) + A(par8_*Ti*Ti*Ti);
    *sc = A(c_w_Pu) * sPu + A(1 - c_w_Pu) * sU;
    *concOut = conc_at_Pu;
}

// ---- thermalExpansionMartinUPuO2.C --------------------------------------
static void martin(scalar Ti, scalar Tref, scalar OM, scalar* val, scalar* sc)
{
    scalar par1_ = 9.828e-6, par2_ = -6.39e-10, par3_ = 1.33e-12, par4_ = -1.757e-17,
           par5_ = 1.1833e-5, par6_ = -5.013e-9, par7_ = 3.756e-12, par8_ = -6.125e-17;
    scalar x_ = 2.0 - OM;
    const scalar alphaRef =                                          // UPSTREAM
    (par1_ + par2_*Tref + par3_*pow(Tref,2) + par4_*pow(Tref,3)) * (1 + 3.98*x_); // UPSTREAM
        const scalar alpha = ( Ti <= 923 )                           // UPSTREAM
        ? (par1_ + par2_*Ti + par3_*pow(Ti,2) + par4_*pow(Ti,3)) * (1 + 3.98*x_) // UPSTREAM
        : (par5_ + par6_*Ti + par7_*pow(Ti,2) + par8_*pow(Ti,3)) * (1 + 3.98*x_); // UPSTREAM
        symmTensor nominalValue = (alpha*Ti-alphaRef*Tref)*I;        // UPSTREAM
    *val = dz(nominalValue);
    const scalar sRef = (A(par1_) + A(par2_*Tref) + A(par3_*Tref*Tref) + A(par4_*Tref*Tref*Tref)) * A(1 + 3.98*x_);
    const scalar sT = (Ti <= 923)
        ? (A(par1_) + A(par2_*Ti) + A(par3_*Ti*Ti) + A(par4_*Ti*Ti*Ti)) * A(1 + 3.98*x_)
        : (A(par5_) + A(par6_*Ti) + A(par7_*Ti*Ti) + A(par8_*Ti*Ti*Ti)) * A(1 + 3.98*x_);
    *sc = sT * Ti + sRef * Tref;
}

// ---- thermalExpansionLemehovUPuO2.C (N_Am = N_Np = 0) --------------------
static void lemehov(scalar Ti, scalar Bu, scalar OM_cell, scalar N_U, scalar N_Pu, scalar* val, scalar* sc, scalar* concOut)
{
    scalar b0_ = -0.3080, b1_ = 3.4303, b2_ = -1.9157, b3_ = 3.4636, by_ = 3.98;
    const scalar OM_[1] = {OM_cell};
    const int cellI = 0;
        const scalar BuGWd_tHM = Bu / 1000;                          // UPSTREAM
        const scalar N_Am = 0.0;
        const scalar N_Np = 0.0;
        const scalar conc_at_Pu = N_Pu / (N_Pu + N_Am + N_Np + N_U); // UPSTREAM
        const scalar conc_at_Am = N_Am / (N_Pu + N_Am + N_Np + N_U); // UPSTREAM
        const scalar k_Pu = 1.13;                                    // UPSTREAM
        const scalar k_Am = 1.12;                                    // UPSTREAM
        const scalar c_w_Pu = conc_at_Pu / k_Pu;                     // UPSTREAM
        const scalar c_w_Am = conc_at_Am / k_Am;                     // UPSTREAM
        const scalar Tm0 = 3147                                      // UPSTREAM
                    -  364.85  * c_w_Pu                              // UPSTREAM
                    -  1014.15 * (2.0-OM_[cellI])                    // UPSTREAM
                    -  329.5   * c_w_Am;                             // UPSTREAM
        const scalar Tm = 2964.94 + (Tm0 - 2964.94) * exp(-BuGWd_tHM/24.25); // UPSTREAM
        const scalar dL_L0 = 0.01 * (b0_                             // UPSTREAM
                              + b1_*Ti/Tm                            // UPSTREAM
                              + b2_*pow(Ti/Tm,2.0)                   // UPSTREAM
                              + b3_*pow(Ti/Tm,3.0));                 // UPSTREAM
        const scalar dL_Ly = dL_L0 * (1+by_*(2-OM_[cellI]));         // UPSTREAM
        symmTensor nominalValue = (dL_Ly) * I;                       // UPSTREAM
    *val = dz(nominalValue);
    const scalar r = Ti / Tm;
    *sc = 0.01 * (A(b0_) + A(b1_*r) + A(b2_*r*r) + A(b3_*r*r*r)) * A(1 + by_*(2 - OM_cell));
    *concOut = conc_at_Pu;
}

// ---- thermalExpansionMAMOX.C ---------------------------------------------
static scalar mamoxA(int i, scalar cPu, scalar x, scalar* abs_sum)
{
    static const scalar B[4][6] = {
        {-2.8809, 0.0301, -4.3954, 0.0156, -15.1759, 2.5642},
        {9.5024, -0.1864, 15.8173, -0.0229, 7.6258, -7.5789},
        {2.0894, 2.9483, -19.9227, -1.0355, 73.8931, 11.6442},
        {4.4096, -1.4263, 23.5638, 0.0251, -54.751, -14.418}};
    const scalar b0 = B[i][0], b1 = B[i][1], b2 = B[i][2], b3 = B[i][3], b4 = B[i][4], b5 = B[i][5];
    const scalar sc[4] = {1e-3, 1e-6, 1e-10, 1e-13};
    scalar r = 0;
    // UPSTREAM a0..a3 (bodies are identical but for b_i and the decade factor)
    if (i == 0) r = 1e-3*(b0 + b1*cPu + b2*x + b3*pow(cPu,2.0) + b4*pow(x,2.0) + b5*cPu*x); // UPSTREAM
    if (i == 1) r = 1e-6*(b0 + b1*cPu + b2*x + b3*pow(cPu,2.0) + b4*pow(x,2.0) + b5*cPu*x); // UPSTREAM
    if (i == 2) r = 1e-10*(b0 + b1*cPu + b2*x + b3*pow(cPu,2.0) + b4*pow(x,2.0) + b5*cPu*x); // UPSTREAM
    if (i == 3) r = 1e-13*(b0 + b1*cPu + b2*x + b3*pow(cPu,2.0) + b4*pow(x,2.0) + b5*cPu*x); // UPSTREAM
    *abs_sum = sc[i] * (A(b0) + A(b1*cPu) + A(b2*x) + A(b3*cPu*cPu) + A(b4*x*x) + A(b5*cPu*x));
    return r;
}
static void mamox(scalar Ti, scalar Tref, scalar cPu_, scalar OM, scalar* val, scalar* sc)
{
    scalar x_ = 2.0 - OM;
    scalar s0, s1, s2, s3;
    const scalar a0v = mamoxA(0, cPu_, x_, &s0), a1v = mamoxA(1, cPu_, x_, &s1),
                 a2v = mamoxA(2, cPu_, x_, &s2), a3v = mamoxA(3, cPu_, x_, &s3);
    scalar dL_L0_ref = a0v                                           // UPSTREAM (a0(cPu_, x_) etc. evaluated once here)
                    + a1v * Tref                                     // UPSTREAM
                    + a2v * pow(Tref, 2.0)                           // UPSTREAM
                    + a3v * pow(Tref, 3.0);                          // UPSTREAM
        scalar dL_L0 = a0v                                           // UPSTREAM
                    + a1v * Ti                                       // UPSTREAM
                    + a2v * pow(Ti, 2.0)                             // UPSTREAM
                    + a3v * pow(Ti, 3.0);                            // UPSTREAM
        symmTensor nominalValue = (dL_L0-dL_L0_ref)*I;               // UPSTREAM
    *val = dz(nominalValue);
    *sc = s0 + s1 * Ti + s2 * Ti * Ti + s3 * Ti * Ti * Ti
        + s0 + s1 * Tref + s2 * Tref * Tref + s3 * Tref * Tref * Tref;
}

// ---- thermalExpansionMatproZy.C (setAlphaT) -------------------------------
static scalar matproZySet(scalar Ti, scalar* sc)
{
    scalar par3 = 6.721e-6, par4 = 2.073e-3, par5 = 9.7e-6, par7 = 9.4e-3;
    scalar zz = 0;
    if (Ti < 1073) // Alpha phase
    {
        zz = par3*Ti - par4;                                         // UPSTREAM
        *sc = A(par3*Ti) + par4;
    }
    else if (Ti < 1273)
    {
        scalar TAlpha = 1073;                                        // UPSTREAM
        scalar TBeta  = 1273;                                        // UPSTREAM
        scalar axAlpha = par3*TAlpha - par4;                         // UPSTREAM
        scalar axBeta  = par5*TBeta - par7;                          // UPSTREAM
        zz = axAlpha*(TBeta - Ti)/(TBeta - TAlpha) + axBeta*(Ti - TAlpha)/(TBeta - TAlpha); // UPSTREAM
        *sc = A(axAlpha) * (TBeta - Ti) / (TBeta - TAlpha) + A(axBeta) * (Ti - TAlpha) / (TBeta - TAlpha)
            + A(par3*TAlpha) + par4 + A(par5*TBeta) + par7;
    }
    else
    {
        zz = par5*Ti - par7;                                         // UPSTREAM
        *sc = A(par5*Ti) + par7;
    }
    return zz;
}
static void matproZy(scalar Ti, scalar Tref, scalar* val, scalar* sc)
{
    scalar sRef, sT;
    const scalar alphaTref = matproZySet(Tref, &sRef);               // (setAlphaT(Tref))
    symmTensor nominalValue(matproZySet(Ti, &sT) - alphaTref);       // (setAlphaT(Ti) - alphaTref)
    *val = dz(nominalValue);
    *sc = sT + sRef;
}

// ---- thermalExpansionGehr1515Ti.C ----------------------------------------
static void gehr(scalar T, scalar* val, scalar* sc)
{
    scalar par1 = -3.101e-4, par2 = 1.545e-5, par3 = 2.75e-9;
        const scalar Ti = T - 273.15;                                // UPSTREAM
        symmTensor nominalValue = (par1 + par2 * Ti + par3*pow(Ti,2))*I; // UPSTREAM
        nominalValue = dz(nominalValue);
        if( T <= 293 )                                               // UPSTREAM
        {
            nominalValue *= 0;                                       // UPSTREAM
        }
    *val = nominalValue;
    *sc = A(par1) + A(par2 * Ti) + A(par3 * Ti * Ti);
}

// ---- thermalExpansionMolybdenum.C ----------------------------------------
static void molybdenum(scalar Ti, scalar* val, scalar* sc)
{
    scalar par1 = 4.985E-06, par2 = 6.667E-10;
        symmTensor nominalValue((par1 + par2*Ti)*(Ti - 273.15)*I);  // UPSTREAM
    *val = dz(nominalValue);
    *sc = A(nominalValue);
}

// ---- thermalExpansionSneadSiC.C ------------------------------------------
static void snead(scalar Ti, scalar Tsf, scalar* val, scalar* sc)
{
    scalar par1 = -1.8276, par2 = 0.0178, par3 = -1.5544e-5, par4 = 4.5246e-9, par5 = 5e-6,
           Treference = 298.15;
        scalar alpha;
        scalar alpha_sf;
        if (Ti <1273.15)                                             // UPSTREAM
        {
          alpha = 1e-6*(par1+par2*Ti+par3*pow(Ti,2)+par4*pow(Ti,3)); // UPSTREAM
        }
        else
        {
          alpha = par5;                                              // UPSTREAM
        }
        alpha_sf = 1e-6*(par1+par2*Tsf+par3*pow(Tsf,2)+par4*pow(Tsf,3)); // UPSTREAM
        symmTensor nominalValue = (alpha*(Ti-Treference)-alpha_sf*(Tsf-Treference)) // UPSTREAM
                                   / (1+alpha_sf*(Tsf-Treference))*I; // UPSTREAM
    *val = dz(nominalValue);
    const scalar sa = (Ti < 1273.15) ? 1e-6 * (A(par1) + A(par2*Ti) + A(par3*Ti*Ti) + A(par4*Ti*Ti*Ti)) : par5;
    const scalar sasf = 1e-6 * (A(par1) + A(par2*Tsf) + A(par3*Tsf*Tsf) + A(par4*Tsf*Tsf*Tsf));
    *sc = (sa * A(Ti - Treference) + sasf * A(Tsf - Treference)) / A(1 + alpha_sf*(Tsf-Treference));
}

// ---- thermalExpansionSwindemanHastelloyN.C -------------------------------
static void swindeman(scalar Ti, scalar Tref, scalar* val, scalar* sc)
{
    scalar par1 = 0.005291, par2 = 9.682, par3 = 107.8;
    const symmTensor referenceValue =                                // UPSTREAM
    (par1*(Tref-273.15)*(Tref-273.15) + par2*(Tref-273.15) + par3)*(1.0e-6)*I; // UPSTREAM
        symmTensor nominalValue =                                    // UPSTREAM
        (par1*(Ti-273.15)*(Ti-273.15) + par2*(Ti-273.15) + par3)*(1.0e-6)*I // UPSTREAM
        - referenceValue;                                            // UPSTREAM
    *val = dz(nominalValue);
    *sc = 1e-6 * (A(par1*(Ti-273.15)*(Ti-273.15)) + A(par2*(Ti-273.15)) + par3
                + A(par1*(Tref-273.15)*(Tref-273.15)) + A(par2*(Tref-273.15)) + par3);
}

// ---- thermalExpansionPARFUMEBuffer.C ---------------------------------------
// returns the unsigned-dead-zone value (as written upstream) and the
// mag()-fixed twin.
static void buffer(scalar T, scalar TrefK, scalar* val_up, scalar* val_fix, scalar* sc)
{
    scalar par1 = 5.0, par2 = 0.11, par3 = 400.0, par4 = 700.0;
        const scalar Ti = T - 273.15;                                // UPSTREAM
        const scalar Tref = TrefK -273.15;                           // UPSTREAM (Tref_.value() -273.15)
        const scalar alpha = par1*(1.0+par2*(Ti-par3)/par4)*1e-6;   // UPSTREAM
        symmTensor nominalValue = alpha*(Ti-Tref)*I;                 // UPSTREAM
    *val_up = dz_unsigned(nominalValue);
    *val_fix = dz(nominalValue);
    *sc = A(nominalValue);
}

// ---- thermalExpansionPARFUMEPyC.C (sphereCoordinate = true) ---------------
static void pyc(scalar Ti, scalar Tref, scalar BAF_, scalar* r_up, scalar* t_up, scalar* r_fix, scalar* t_fix, scalar* sc)
{
    scalar par1 = 30.0, par2 = 37.5, par3 = 0.11, par4 = 673.0, par5 = 700.0, par6 = 36.0;
    const scalar Rr = 2.0 / (2.0 + BAF_);                            // UPSTREAM
    const scalar Rt = (1.0 + BAF_) / (2.0 + BAF_);                   // UPSTREAM
        const scalar alpha_r = (par1-par2*Rr)*(1.0+par3*(Ti-par4)/par5)*1e-6; // UPSTREAM
        const scalar alpha_t = (par6*pow(Rt-1.0,2.0)+1.0)*(1.0+par3*(Ti-par4)/par5)*1e-6; // UPSTREAM
        const scalar xx = alpha_r*(Ti-Tref);   // nominalValue_sphere = alpha_r*(Ti-Tref)*I
        const scalar yy = alpha_t*(Ti-Tref);                         // (nominalValue_sphere.yy() = alpha_t*(Ti-Tref);)
        const scalar zz = alpha_t*(Ti-Tref);
    const bool zero_up = (xx < 1e-7 && yy < 1e-7 && zz < 1e-7);
    const bool zero_fix = (mag(xx) < 1e-7 && mag(yy) < 1e-7 && mag(zz) < 1e-7);
    *r_up = zero_up ? 0.0 : xx;
    *t_up = zero_up ? 0.0 : yy;
    *r_fix = zero_fix ? 0.0 : xx;
    *t_fix = zero_fix ? 0.0 : yy;
    *sc = std::max(A(xx), A(yy));
}

// ---- thermalExpansionPARFUMESiC.C -----------------------------------------
static void sic(scalar Ti, scalar Tref, scalar alpha_, scalar* val, scalar* sc)
{
        symmTensor nominalValue = alpha_ * (Ti-Tref)*I;              // UPSTREAM
    *val = dz(nominalValue);
    *sc = A(nominalValue);
}

static void emit(const char* name, scalar T, scalar Tref, scalar a, scalar b, scalar c, scalar d, scalar scale, scalar v)
{
    std::printf("%s %.17g %.17g %.17g %.17g %.17g %.17g %.17g %.17g\n", name, T, Tref, a, b, c, d, scale, v);
}

int main()
{
    const scalar Ts[] = {273, 290, 293, 293.15, 300, 333.33, 400, 500, 600, 700, 800, 900, 923, 923.5,
        1000, 1073, 1085.5, 1100, 1200, 1273, 1273.15, 1274, 1300, 1500, 1700, 1800, 2000,
        2400, 2800, 3000};
    const scalar Trefs[] = {293.15, 300, 600};
    const scalar OMs[] = {1.94, 1.97, 2.0};
    // (N_U, N_Pu) pairs
    const scalar NUs[] = {1.0, 19.0, 3.0};
    const scalar NPus[] = {0.0, 1.0, 1.0};
    const scalar Bus[] = {0, 5e3, 3e4, 1e5};
    const scalar cPus[] = {0.1, 0.3};
    const scalar BAFs[] = {1.0, 1.05, 1.3};
    const scalar alphas[] = {1e-5, 1.7e-5};
    const scalar siCalphas[] = {4.9e-6, 5.5e-6};
    scalar v, s, v2, v3, v4, c;

    for (scalar T : Ts)
    {
        gehr(T, &v, &s);                    emit("Gehr1515Ti", T, 0, 0, 0, 0, 0, s, v);
        molybdenum(T, &v, &s);              emit("Molybdenum", T, 0, 0, 0, 0, 0, s, v);
        for (int i = 0; i < 3; i++)
        {
            matproUPuO2(T, NUs[i], NPus[i], &v, &s, &c);
            emit("MatproUPuO2", T, 0, c, 0, 0, 0, s, v);
        }
        for (scalar Tref : Trefs)
        {
            for (scalar al : alphas) { constant(T, Tref, al, &v, &s); emit("Constant", T, Tref, al, 0, 0, 0, s, v); }
            for (scalar al : siCalphas) { sic(T, Tref, al, &v, &s); emit("PARFUMESiC", T, Tref, al, 0, 0, 0, s, v); }
            relap(T, Tref, &v, &s);         emit("RelapUO2", T, Tref, 0, 0, 0, 0, s, v);
            for (scalar OM : OMs) { martin(T, Tref, OM, &v, &s); emit("MartinUPuO2", T, Tref, OM, 0, 0, 0, s, v); }
            for (scalar cPu : cPus)
                for (scalar OM : OMs) { mamox(T, Tref, cPu, OM, &v, &s); emit("MAMOX", T, Tref, cPu, OM, 0, 0, s, v); }
            if (T >= 273 && T <= 1800) { matproZy(T, Tref, &v, &s); emit("MatproZy", T, Tref, 0, 0, 0, 0, s, v); }
            snead(T, Tref, &v, &s);         emit("SneadSiC", T, Tref, 0, 0, 0, 0, s, v);
            swindeman(T, Tref, &v, &s);     emit("SwindemanHastelloyN", T, Tref, 0, 0, 0, 0, s, v);
            buffer(T, Tref, &v, &v2, &s);
            emit("PARFUMEBuffer_up", T, Tref, 0, 0, 0, 0, s, v);
            emit("PARFUMEBuffer_fix", T, Tref, 0, 0, 0, 0, s, v2);
            for (scalar baf : BAFs)
            {
                pyc(T, Tref, baf, &v, &v2, &v3, &v4, &s);
                emit("PARFUMEPyC_r_up", T, Tref, baf, 0, 0, 0, s, v);
                emit("PARFUMEPyC_t_up", T, Tref, baf, 0, 0, 0, s, v2);
                emit("PARFUMEPyC_r_fix", T, Tref, baf, 0, 0, 0, s, v3);
                emit("PARFUMEPyC_t_fix", T, Tref, baf, 0, 0, 0, s, v4);
            }
        }
        for (int i = 0; i < 3; i++)
            for (scalar OM : OMs)
                for (scalar Bu : Bus)
                {
                    lemehov(T, Bu, OM, NUs[i], NPus[i], &v, &s, &c);
                    emit("LemehovUPuO2", T, 0, c, OM, Bu, 0, s, v);
                }
    }
    return 0;
}
