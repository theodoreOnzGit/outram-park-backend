// gh:#1367 row 3. Evaluates the interfacial heat-transfer K() expressions of
// upstream OpenFOAM on frozen scalarFields and dumps inputs and results as
// IEEE-754 bit patterns (hex), one "name index hex" line each.
//
// Expressions are transcribed verbatim from
//   ESI v2506  src/phaseSystemModels/twoPhaseEuler/twoPhaseSystem/
//              interfacialModels/heatTransferModels/{RanzMarshall,sphericalHeatTransfer}
//   Foundation OpenFOAM-dev 852527377c7e applications/modules/multiphaseEuler/
//              phaseSystem/interfacialModels/heatTransferModels/{sphericalHeatTransfer,Gunn}
// Only the phasePair accessors are replaced by plain fields (the phaseSystem
// library is not built). Field operators, max, sqrt, cbrt, pow, sqr are
// OpenFOAM's own.
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
        std::printf("%s %d %016llx\n", name, int(i), (unsigned long long)b);
    }
}

int main()
{
    const label n = 4096;
    scalarField ad(n), kc(n), kd(n), d(n), Re(n), Pr(n);
    for (label i = 0; i < n; ++i)
    {
        const double u0 = uniform01(), u1 = uniform01(), u2 = uniform01();
        const double u3 = uniform01(), u4 = uniform01(), u5 = uniform01();
        ad[i] = 0.0 + (1.0 - 0.0)*u0;
        kc[i] = 0.02 + (0.7 - 0.02)*u1;
        kd[i] = 0.01 + (0.1 - 0.01)*u2;
        d[i] = 1e-4 + (1e-2 - 1e-4)*u3;
        Re[i] = 0.0 + (5e4 - 0.0)*u4;
        Pr[i] = 0.5 + (12.0 - 0.5)*u5;
    }
    ad[0] = 0.0;
    ad[1] = 1.0;
    Re[2] = 0.0;
    ad[3] = 1e-9;
    const scalarField ac(1.0 - ad);
    const scalar residualAlpha = 1e-6;

    // ESI v2506 RanzMarshall::K
    scalarField Nu(scalar(2) + 0.6*sqrt(Re)*cbrt(Pr));
    const scalarField Krm(6.0*max(ad, residualAlpha)*kc*Nu/sqr(d));

    // ESI v2506 sphericalHeatTransfer::K: continuous().kappa()
    const scalarField KsphEsi(60.0*max(ad, residualAlpha)*kc/sqr(d));

    // Foundation sphericalHeatTransfer::K: dispersed().thermo().kappa()
    const scalarField KsphFound(60.0*max(ad, residualAlpha)*kd/sqr(d));

    // Foundation Gunn::K (residualAlpha of the continuous phase = same value)
    const scalarField alpha2(max(ac, residualAlpha));
    const scalarField sqrAlpha2(sqr(alpha2));
    const scalarField NuG
    (
        (7 - 10*alpha2 + 5*sqrAlpha2)
       *(1 + 0.7*pow(Re, 0.2)*cbrt(Pr))
      + (1.33 - 2.4*alpha2 + 1.2*sqrAlpha2)
       *pow(Re, 0.7)*cbrt(Pr)
    );
    const scalarField Kgunn(6*max(ad, residualAlpha)*kc*NuG/sqr(d));

    dump("ad", ad); dump("kc", kc); dump("kd", kd); dump("d", d);
    dump("Re", Re); dump("Pr", Pr);
    dump("sqrtRe", sqrt(Re)); dump("cbrtPr", cbrt(Pr));
    dump("powRe02", pow(Re, 0.2)); dump("powRe07", pow(Re, 0.7));
    dump("Nu_rm", Nu); dump("Nu_gunn", NuG);
    dump("K_rm_esi", Krm);
    dump("K_sph_esi", KsphEsi);
    dump("K_sph_found", KsphFound);
    dump("K_gunn_found", Kgunn);
    return 0;
}
