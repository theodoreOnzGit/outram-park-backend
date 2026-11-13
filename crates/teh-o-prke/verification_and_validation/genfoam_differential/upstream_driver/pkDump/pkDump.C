// pkDump: gh:#1439. Subclasses GeN-Foam 652b3da's solvers::pointKineticNeutronics
// (members are protected) and (F) runs the UNMODIFIED include/correctReactivity.H
// on fixed inputs, (K) drives the real correctPhysics() step by step.
// Original OUTRAM PARK verification code; links, does not modify, upstream.
#include "fvCFD.H"
#include "pointKineticNeutronics.H"
#include <cstdio>
#include <cmath>
#include <vector>
#include <utility>

namespace Foam {

class probe : public solvers::pointKineticNeutronics
{
public:
    probe(fvMesh& m) : solvers::pointKineticNeutronics(m) {}

    void setMap(const List<Pair<scalar>>& m) { controlRodReactivityMap_ = m; }
    void setCoeffs(scalar dop, scalar fuel, scalar clad, scalar cool,
                   scalar rho, scalar st, scalar stm, scalar dl)
    {
        coeffDoppler_ = dop; coeffTFuel_ = fuel; coeffTClad_ = clad;
        coeffTCool_ = cool; coeffRhoCool_ = rho; coeffTStruct_ = st;
        coeffTStructMech_ = stm; coeffDrivelineExp_ = dl;
    }
    void setRefs(scalar f, scalar c, scalar co, scalar r, scalar s, scalar sm, scalar d)
    {
        TFuelRef_ = f; TCladRef_ = c; TCoolRef_ = co; rhoCoolRef_ = r;
        TStructRef_ = s; TStructMechRef_ = sm; TDrivelineRef_ = d;
    }
    void printCoeffs(const char* tag) const
    {
        printf("COEF %s doppler %.17g\nCOEF %s fuel %.17g\nCOEF %s clad %.17g\n"
               "COEF %s cool %.17g\nCOEF %s rho %.17g\nCOEF %s struct %.17g\n"
               "COEF %s structmech %.17g\nCOEF %s driveline %.17g\n",
               tag, coeffDoppler_, tag, coeffTFuel_, tag, coeffTClad_, tag, coeffTCool_,
               tag, coeffRhoCool_, tag, coeffTStruct_, tag, coeffTStructMech_,
               tag, coeffDrivelineExp_);
        printf("REF %s %.17g %.17g %.17g %.17g %.17g %.17g %.17g\n", tag,
               TFuelRef_, TCladRef_, TCoolRef_, rhoCoolRef_, TStructRef_,
               TStructMechRef_, TDrivelineRef_);
        forAll(controlRodReactivityMap_, i)
            printf("MAP %s %.17g %.17g\n", tag, controlRodReactivityMap_[i].first(),
                   controlRodReactivityMap_[i].second());
        printf("KIN %s %.17g", tag, promptGenerationTime_);
        printf(" %d", int(betas_.size()));
        forAll(betas_, i) printf(" %.17g", betas_[i]);
        forAll(lambdas_, i) printf(" %.17g", lambdas_[i]);
        printf("\n");
    }

    // Upstream's correctReactivity.H, verbatim, on fixed inputs.
    void feedback(const char* tag, int idx, scalar TFuelValue, scalar TCladValue,
                  scalar TCoolValue, scalar rhoCoolValue, scalar TStructValue,
                  scalar TStructMechValue, scalar TDrivelineValue)
    {
        #include "correctReactivity.H"
        printf("FB %s %d %.17g %.17g %.17g %.17g %.17g %.17g %.17g | %.17g %.17g "
               "%.17g %.17g %.17g %.17g %.17g %.17g %.17g\n",
               tag, idx, TFuelValue, TCladValue, TCoolValue, rhoCoolValue,
               TStructValue, TStructMechValue, TDrivelineValue,
               DopplerReactivity, TFuelReactivity, TCladReactivity, TCoolReactivity,
               rhoCoolReactivity, TStructReactivity, TStructMechReactivity,
               drivelineReactivity, totalReactivity_);
    }

    void setFields(scalar tf, scalar tc, scalar tco, scalar rc, scalar ts)
    {
        TFuel_ = dimensionedScalar("", dimTemperature, tf);
        TClad_ = dimensionedScalar("", dimTemperature, tc);
        TCool_ = dimensionedScalar("", dimTemperature, tco);
        rhoCool_ = dimensionedScalar("", dimDensity, rc);
        TStruct_ = dimensionedScalar("", dimTemperature, ts);
    }

    void stepAndPrint(int idx, scalar dt)
    {
        const scalar pOld = fissionPower_;
        scalarList cOld(precursorPowers_);
        correctPhysics();
        printf("K %d %.17g %.17g %.17g", idx, dt, totalReactivity_, pOld);
        forAll(cOld, i) printf(" %.17g", cOld[i]);
        printf(" | %.17g", fissionPower_);
        forAll(precursorPowers_, i) printf(" %.17g", precursorPowers_[i]);
        printf("\n");
        fflush(stdout);
    }
};

static List<Pair<scalar>> mkMap(const std::vector<std::pair<double, double>>& v)
{
    List<Pair<scalar>> m(label(v.size()));
    for (size_t i = 0; i < v.size(); ++i) m[i] = Pair<scalar>(v[i].first, v[i].second);
    return m;
}

} // namespace Foam

using namespace Foam;

int main(int argc, char* argv[])
{
    #include "setRootCase.H"
    #include "createTime.H"
    fvMesh mesh
    (
        IOobject("neutroRegion", runTime.timeName(), runTime, IOobject::MUST_READ)
    );
    probe pk(mesh);

    const std::vector<std::pair<double, double>> fftfMap =
    {
        {-0.1113, 1000e-5}, {-0.0613, 600e-5}, {-0.0113, 100e-5}, {0, 0},
        {0.0387, -400e-5}, {0.0887, -1000e-5}, {0.1387, -1600e-5}
    };
    std::vector<std::pair<double, double>> fftfDesc(fftfMap.rbegin(), fftfMap.rend());
    const std::vector<std::pair<double, double>> dyadicAsc =
    {
        {-0.25, 0.5}, {-0.125, 0.25}, {0, 0}, {0.125, -0.25}, {0.25, -0.75}
    };
    std::vector<std::pair<double, double>> dyadicDesc(dyadicAsc.rbegin(), dyadicAsc.rend());

    // ---- F1: FFTF coefficient set (2D_FFTF/nuclearData), 400-step history
    pk.setCoeffs(5.0e-3, -4.0e-6, -1.0e-6, 0.0, 1.5e-5, -1.0e-5, 0.0, 0.00007);
    pk.setMap(mkMap(fftfMap));
    pk.setRefs(1000.0, 800.0, 750.0, 820.0, 700.0, 0.0, 650.0);
    pk.printCoeffs("F1");
    for (int i = 0; i < 400; ++i)
    {
        const double s = std::sin(0.037 * i), c = std::cos(0.051 * i);
        double tf = 1000.0 + 450.0 * s;
        if (i % 97 == 0) tf = 0.5e-15 * (1 + i);   // below SMALL: exercises the floor
        if (i == 5) tf = 1000.0;                   // exactly at reference
        pk.feedback("F1", i, tf, 800.0 + 60.0 * c, 750.0 + 40.0 * s,
                    820.0 + 15.0 * c, 700.0 + 90.0 * s * c, 0.0, 650.0 + 1700.0 * s);
    }
    // ---- F1D: same set, descending map
    pk.setMap(mkMap(fftfDesc));
    pk.printCoeffs("F1D");
    for (int i = 0; i < 400; ++i)
    {
        const double s = std::sin(0.037 * i);
        pk.feedback("F1D", i, 1000.0 + 450.0 * s, 800.0, 750.0, 820.0, 700.0, 0.0,
                    650.0 + 1700.0 * s);
    }
    // ---- F2/F2D: dyadic driveline (coefficient 0.5 m/K) hitting knots and both ends
    pk.setCoeffs(2.5e-3, -3.0e-6, -2.0e-6, 1.0e-6, 2.0e-5, -5.0e-6, 0.0, 0.5);
    pk.setRefs(1200.0, 900.0, 800.0, 810.0, 700.0, 0.0, 600.0);
    pk.setMap(mkMap(dyadicAsc));
    pk.printCoeffs("F2");
    const double dl[] = {-1.0, -0.6, -0.5, -0.4, -0.25, -0.1, 0.0, 0.1, 0.25, 0.3, 0.5, 0.6, 1.0, 1e-9, -1e-9};
    for (int i = 0; i < 15; ++i)
        pk.feedback("F2", i, 1200.0 + 10.0 * i, 900.0, 800.0, 810.0, 700.0, 0.0, 600.0 + dl[i]);
    pk.setMap(mkMap(dyadicDesc));
    pk.printCoeffs("F2D");
    for (int i = 0; i < 15; ++i)
        pk.feedback("F2D", i, 1200.0 + 10.0 * i, 900.0, 800.0, 810.0, 700.0, 0.0, 600.0 + dl[i]);

    // ---- K: real correctPhysics(), FFTF set, uniform fields, varied dt
    pk.setCoeffs(5.0e-3, -4.0e-6, -1.0e-6, 0.0, 1.5e-5, -1.0e-5, 0.0, 0.00007);
    pk.setMap(mkMap(fftfMap));
    pk.printCoeffs("K");
    for (int i = 0; i < 300; ++i)
    {
        const double dt = std::pow(10.0, -3.0 + 2.0 * (i % 50) / 49.0);
        runTime.setDeltaT(dt);
        runTime++;
        const double s = std::sin(0.045 * i), c = std::cos(0.07 * i);
        pk.setFields(1000.0 + 120.0 * s, 800.0 + 30.0 * c, 750.0 + 20.0 * s,
                     820.0 + 10.0 * c, 700.0 + 40.0 * s);
        pk.stepAndPrint(i, dt);
    }
    return 0;
}
