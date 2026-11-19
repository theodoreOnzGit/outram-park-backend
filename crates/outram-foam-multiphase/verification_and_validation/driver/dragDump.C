// gh:#1367 row 2. Evaluates the twoPhaseEuler interfacial-drag expressions of
// ESI OpenFOAM-v2506 on frozen fields and dumps inputs and results as IEEE-754
// bit patterns (hex), one "name index hex" line each.
//
// Expressions are transcribed verbatim from ESI v2506
//   src/phaseSystemModels/twoPhaseEuler/twoPhaseSystem/
//     phasePair/phasePair.C                    Re()  = magUr()*d/nu
//     interfacialModels/dragModels/dragModel/dragModel.C   Ki(), K()
//     interfacialModels/dragModels/SchillerNaumann/SchillerNaumann.C   CdRe()
//     interfacialModels/dragModels/WenYu/WenYu.C                       CdRe()
//     interfacialModels/swarmCorrections/noSwarm/noSwarm.C             Cs() = 1
// Only the phasePair/phase accessors are replaced by plain fields (the
// phaseSystem library is not built). Field operators, neg, pos0, max, sqrt,
// pow, sqr are OpenFOAM's own. Constants: twoPhaseEulerFoam/laminar/bubbleColumn
// (d 3e-3, residualRe 1e-3, residualAlpha 1e-6; water rho 1027, mu 3.645e-4).
#include "vectorField.H"
#include "scalarField.H"
#include <cstdio>
#include <cstdint>
#include <cstring>

using namespace Foam;

static uint64_t state = 0x9E3779B97F4A7C15ULL;

static double uniform01()
{
    state += 0x9E3779B97F4A7C15ULL;
    uint64_t z = state;
    z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9ULL;
    z = (z ^ (z >> 27)) * 0x94D049BB133111EBULL;
    z ^= z >> 31;
    return double(z >> 11) * (1.0 / 9007199254740992.0);
}

static void dump(const char* name, const scalarField& f)
{
    for (label i = 0; i < f.size(); ++i)
    {
        uint64_t b;
        double v = f[i];
        std::memcpy(&b, &v, sizeof(b));
        std::printf("%s %d %016llx\n", name, int(i), static_cast<unsigned long long>(b));
    }
}

int main()
{
    const label n = 4096;
    scalarField ad(n);
    vectorField Ud(n), Uc(n);
    for (label i = 0; i < n; ++i)
    {
        const double u0 = uniform01();
        double ud[3], uc[3];
        for (int k = 0; k < 3; ++k) ud[k] = uniform01();
        for (int k = 0; k < 3; ++k) uc[k] = uniform01();
        const double s = uniform01();
        ad[i] = u0;
        Ud[i] = vector((2*ud[0]-1)*0.6*s, (2*ud[1]-1)*0.6*s, (2*ud[2]-1)*0.6*s);
        Uc[i] = vector((2*uc[0]-1)*0.6*s, (2*uc[1]-1)*0.6*s, (2*uc[2]-1)*0.6*s);
    }
    Uc[0] = Ud[0];          // Ur = 0, Re = 0
    ad[1] = 0.0;
    ad[2] = 1.0;            // continuous fraction below the residual
    ad[3] = 1e-9;

    const scalar d = 3e-3;
    const scalar rhoc = 1027.0;
    const scalar nuc = 3.645e-4/1027.0;
    const scalar residualAlpha = 1e-6;
    const scalar residualRe = 1e-3;
    const scalarField ac(1.0 - ad);        // alpha_c = 1 - alpha_d

    // phasePair::magUr, Re
    const scalarField magUr(mag(Ud - Uc));
    const scalarField Re(magUr*d/nuc);

    // SchillerNaumann::CdRe
    const scalarField CdReSN
    (
        neg(Re - 1000)*24.0*(1.0 + 0.15*pow(Re, 0.687))
      + pos0(Re - 1000)*0.44*max(Re, residualRe)
    );

    // WenYu::CdRe
    const scalarField alpha2(max(scalar(1) - ad, residualAlpha));
    const scalarField Res(alpha2*Re);
    const scalarField CdsRes
    (
        neg(Res - 1000)*24.0*(1.0 + 0.15*pow(Res, 0.687))
      + pos0(Res - 1000)*0.44*max(Res, residualRe)
    );
    const scalarField CdReWY
    (
        CdsRes*pow(alpha2, -3.65)*max(ac, residualAlpha)
    );

    // noSwarm::Cs
    const scalarField Cs(n, 1.0);

    // dragModel::Ki, K
    const scalarField KiSN(0.75*CdReSN*Cs*rhoc*nuc/sqr(d));
    const scalarField KSN(max(ad, residualAlpha)*KiSN);
    const scalarField KiWY(0.75*CdReWY*Cs*rhoc*nuc/sqr(d));
    const scalarField KWY(max(ad, residualAlpha)*KiWY);

    dump("ad", ad);
    for (int k = 0; k < 3; ++k)
    {
        const char* nm[3] = {"Ud_x", "Ud_y", "Ud_z"};
        const char* nc[3] = {"Uc_x", "Uc_y", "Uc_z"};
        dump(nm[k], Ud.component(k));
        dump(nc[k], Uc.component(k));
    }
    dump("magUr", magUr);
    dump("Re", Re);
    dump("powRe0687", pow(Re, 0.687));
    dump("powRes0687", pow(Res, 0.687));
    dump("powA2_365", pow(alpha2, -3.65));
    dump("CdRe_sn", CdReSN);
    dump("CdRe_wy", CdReWY);
    dump("K_sn", KSN);
    dump("K_wy", KWY);
    return 0;
}
