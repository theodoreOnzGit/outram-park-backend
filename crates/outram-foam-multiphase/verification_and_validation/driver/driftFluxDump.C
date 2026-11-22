// gh:#1367 row 1. Runs ESI OpenFOAM-v2506's OWN driftFluxFoam classes
// (incompressibleTwoPhaseInteractingMixture, mixtureViscosityModels,
// relativeVelocityModel/simple/general; compiled in unmodified, see
// scripts/run_drift_flux.sh) on the RAS/dahl tutorial mesh with a frozen,
// generated alpha.sludge, and dumps inputs and results as IEEE-754 bit
// patterns (hex), one "name index hex" line each.
//
// Nothing of upstream is transcribed here: correct(), rho() and tauDm() are
// the real upstream member functions.
#include "fvCFD.H"
#include "incompressibleTwoPhaseInteractingMixture.H"
#include "relativeVelocityModel.H"
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

static void dumpValue(const char* name, label i, double v)
{
    uint64_t b;
    std::memcpy(&b, &v, sizeof(b));
    std::printf("%s %d %016llx\n", name, int(i), (unsigned long long)b);
}

static void dumpScalar(const char* name, const scalarField& f)
{
    forAll(f, i) { dumpValue(name, i, f[i]); }
}

static void dumpVector(const char* name, const vectorField& f)
{
    const char* sfx[3] = {"x", "y", "z"};
    for (int c = 0; c < 3; ++c)
    {
        const std::string n = std::string(name) + "_" + sfx[c];
        forAll(f, i) { dumpValue(n.c_str(), i, f[i][c]); }
    }
}

static void dumpSymm(const char* name, const symmTensorField& f)
{
    const char* sfx[6] = {"xx", "xy", "xz", "yy", "yz", "zz"};
    for (int c = 0; c < 6; ++c)
    {
        const std::string n = std::string(name) + "_" + sfx[c];
        forAll(f, i) { dumpValue(n.c_str(), i, f[i][c]); }
    }
}

int main(int argc, char* argv[])
{
    #include "setRootCase.H"
    #include "createTime.H"
    #include "createMesh.H"

    volVectorField U
    (
        IOobject("U", runTime.timeName(), mesh, IOobject::MUST_READ, IOobject::NO_WRITE),
        mesh
    );
    #include "createPhi.H"

    incompressibleTwoPhaseInteractingMixture mixture(U, phi);
    volScalarField& alpha1(mixture.alpha1());
    volScalarField& alpha2(mixture.alpha2());

    // Frozen, generated alpha (the same generator is reproduced by the Rust test)
    const label n = mesh.nCells();
    scalarField a(n);
    for (label i = 0; i < n; ++i)
    {
        a[i] = 0.0 + 0.5*uniform01();
    }
    a[0] = 0.0;
    a[1] = -1e-3;
    a[2] = 1e-9;
    a[3] = 0.5;
    a[4] = 0.999999;  // alpha = 1 gives betac = 0 and tauDm divides 0/0 (upstream traps SIGFPE)
    a[5] = 1e-3;
    alpha1.primitiveFieldRef() = a;
    alpha2 = scalar(1) - alpha1;

    dumpScalar("alpha", alpha1.primitiveField());
    dumpScalar("alpha2", alpha2.primitiveField());
    std::printf("const rhod %016llx\n", [&]{ uint64_t b; double v = mixture.rhod().value(); std::memcpy(&b,&v,8); return (unsigned long long)b; }());
    std::printf("const rhoc %016llx\n", [&]{ uint64_t b; double v = mixture.rhoc().value(); std::memcpy(&b,&v,8); return (unsigned long long)b; }());
    dumpScalar("rho_mix", mixture.rho()().primitiveField());

    // 1. dahl dictionary as shipped: relativeVelocityModel simple
    {
        autoPtr<relativeVelocityModel> m(relativeVelocityModel::New(mixture, mixture));
        m->correct();
        dumpScalar("rho_rvm", m->rho()().primitiveField());
        dumpVector("Udm_simple", m->Udm().primitiveField());
        dumpSymm("tauDm_simple", m->tauDm()().primitiveField());
    }

    // 2. the same dictionary, model general (residualAlpha 0 as in dahl)
    {
        dictionary d(static_cast<const dictionary&>(mixture));
        d.set("relativeVelocityModel", word("general"));
        autoPtr<relativeVelocityModel> m(relativeVelocityModel::New(d, mixture));
        m->correct();
        dumpVector("Udm_general", m->Udm().primitiveField());
    }

    // 3. general with residualAlpha 1e-3
    {
        dictionary coeffs;
        coeffs.add("V0", vector(0, -0.002198, 0));
        coeffs.add("a", 285.84);
        coeffs.add("a1", 0.1);
        coeffs.add("residualAlpha", 1e-3);
        dictionary d;
        d.add("relativeVelocityModel", word("general"));
        d.add("generalCoeffs", coeffs);
        autoPtr<relativeVelocityModel> m(relativeVelocityModel::New(d, mixture));
        m->correct();
        dumpVector("Udm_general_res", m->Udm().primitiveField());
    }

    // 4. transcendental intermediates, evaluated with OpenFOAM's own field
    //    operators (glibc pow/exp), to name the first divergent operation
    {
        const scalarField ap(max(a, scalar(0)));
        dumpScalar("pow10_a", pow(scalar(10), -285.84*ap));
        const scalarField x0(max(a - scalar(0), scalar(0)));
        const scalarField x1(max(a - scalar(1e-3), scalar(0)));
        dumpScalar("exp_a_r0", exp(-285.84*x0));
        dumpScalar("exp_a1_r0", exp(-0.1*x0));
        dumpScalar("exp_a_r1", exp(-285.84*x1));
        dumpScalar("exp_a1_r1", exp(-0.1*x1));
    }
    return 0;
}
