// gh:#1362 row 5 (wall-function slice): nutkWallFunction against the UNMODIFIED
// OpenFOAM-v2506 (ESI) libturbulenceModels. `wallFunctionCoefficients` is the real
// library class (it computes yPlusLam); the stepwise nut of
// nutkWallFunctionFvPatchScalarField::calcNut() is transcribed line for line for
// one face (its lambdas nutLog, the STEPWISE switch and the final `nutw -= nutVis`),
// because the patch field itself needs a case, a mesh and a turbulence model.
// Every double is printed as its hex bit pattern.
#include "wallFunctionCoefficients.H"
#include "scalar.H"
#include "dictionary.H"
#include "IStringStream.H"
#include "mathematicalConstants.H"
#include <cstdint>
#include <cstdio>
#include <cstring>

using namespace Foam;

static void hex(const char* what, const double* v, int n)
{
    std::printf("%s", what);
    for (int k = 0; k < n; k++)
    {
        std::uint64_t u;
        std::memcpy(&u, &v[k], 8);
        std::printf(" %016llx", (unsigned long long)u);
    }
    std::printf("\n");
}

int main()
{
    // (kappa, E): the default, and two others
    const double kappaE[3][2] = {{0.41, 9.8}, {0.40, 9.0}, {0.38, 5.5}};
    for (int c = 0; c < 3; c++)
    {
        const scalar kappa = kappaE[c][0];
        const scalar E = kappaE[c][1];
        char buf[128];
        std::snprintf(buf, sizeof buf, "Cmu 0.09; kappa %.17g; E %.17g;", kappa, E);
        IStringStream is(buf);
        dictionary dict(is);
        const wallFunctionCoefficients wc(dict);
        const scalar yPlusLam = wc.yPlusLam();
        {
            double v[3] = {kappa, E, yPlusLam};
            hex("yplus_lam", v, 3);
        }

        const double nus[3] = {1.0e-5, 1.5e-5, 1.0e-6};
        const double yps[12] =
            {1.0, 5.0, 10.0, 11.0, 11.4, 11.6, 12.0, 30.0, 100.0, 300.0, 1000.0, 5000.0};
        for (int i = 0; i < 3; i++)
        {
            for (int j = 0; j < 12; j++)
            {
                const scalar nutVis = nus[i];
                const scalar yPlusFace = yps[j];

                // nutLog lambda of calcNut()
                const scalar nutLog =
                (
                    nutVis*yPlusFace*kappa
                  / Foam::log(max(E*yPlusFace, 1 + 1e-4))
                );

                // blenderType::STEPWISE
                scalar nutw = 0;
                if (yPlusFace > yPlusLam)
                {
                    nutw = nutLog;
                }
                else
                {
                    nutw = nutVis;
                }
                nutw -= nutVis;

                double v[6] = {kappa, E, nutVis, yPlusFace, nutLog, nutw};
                hex("nutk", v, 6);
            }
        }
    }
    return 0;
}
