/*---------------------------------------------------------------------------*\
  gh:#1362 row 5 differential driver. OUTRAM PARK's own code; it only CALLS
  upstream OpenFOAM-v2506 (ESI, GPL-3.0-or-later, https://www.openfoam.com).

  What it does, for ONE model per run (-model kEpsilon | kOmega | kOmegaSST |
  SpalartAllmaras | Smagorinsky; the case's constant/momentumTransport must
  select the same model):

   1. builds frozen fields on the case mesh from POLYNOMIALS (no libm): U, phi,
      k, epsilon, omega, nuTilda; writes k/epsilon/omega/nuTilda/nut to 0/ so
      the REAL model constructs itself exactly as in an application;
   2. constructs the real incompressible::turbulenceModel and calls validate()
      (the real correctNut) and then the real correct() with every scalar
      solver set to `dumpSolver` (libdumpSolver.so): the assembled matrix is
      printed instead of solved, together with every registered internal field
      alive inside correct() (the model's own G, GbyNu, ...);
   3. prints the real nut(), the real fvc::grad(U) and fvc::div(phi) fields;
   4. TRANSCRIBES the model's source terms from the upstream .C file using the
      real fvc/fvm operators on the real fields (so each term is a named,
      printable per-cell quantity), re-assembles the transcribed equation, and
      AUDITS it bit for bit against the matrix the real model assembled. A
      transcription that does not reproduce the real matrix would be visible in
      the `audit` lines.

  Output: "<key> <index> <hex64> ..." one value per line (IEEE-754 bit patterns).
\*---------------------------------------------------------------------------*/

#include "fvCFD.H"
#include "singlePhaseTransportModel.H"
#include "turbulentTransportModel.H"
#include "wallDist.H"
#include "dumpSolver.H"
#include "fixedValueFvPatchFields.H"
#include "zeroGradientFvPatchFields.H"
#include "calculatedFvPatchFields.H"
#include "simpleFilter.H"
#include "laplaceFilter.H"
#include "anisotropicFilter.H"
#include <cstdio>
#include <cstring>
#include <cstdint>
#include <string>

using namespace Foam;

static void H(const std::string& key, label i, scalar a)
{
    uint64_t u;
    std::memcpy(&u, &a, 8);
    std::printf("%s %d %016llx\n", key.c_str(), int(i), static_cast<unsigned long long>(u));
}

static void H3(const std::string& key, label i, const vector& v)
{
    uint64_t u[3];
    scalar c[3] = {v.x(), v.y(), v.z()};
    std::memcpy(u, c, 24);
    std::printf
    (
        "%s %d %016llx %016llx %016llx\n", key.c_str(), int(i),
        static_cast<unsigned long long>(u[0]),
        static_cast<unsigned long long>(u[1]),
        static_cast<unsigned long long>(u[2])
    );
}

static void H9(const std::string& key, label i, const tensor& t)
{
    uint64_t u[9];
    scalar c[9] = {t.xx(), t.xy(), t.xz(), t.yx(), t.yy(), t.yz(), t.zx(), t.zy(), t.zz()};
    std::memcpy(u, c, 72);
    std::printf("%s %d", key.c_str(), int(i));
    for (int k = 0; k < 9; ++k) std::printf(" %016llx", static_cast<unsigned long long>(u[k]));
    std::printf("\n");
}

static void dumpS(const std::string& key, const scalarField& s)
{
    forAll(s, i) H(key, i, s[i]);
}

static void dumpV(const std::string& key, const vectorField& s)
{
    forAll(s, i) H3(key, i, s[i]);
}

static void dumpT(const std::string& key, const tensorField& s)
{
    forAll(s, i) H9(key, i, s[i]);
}


// symmTensor: xx xy xz yy yz zz
static void H6(const std::string& key, label i, const symmTensor& t)
{
    uint64_t u[6];
    scalar c[6] = {t.xx(), t.xy(), t.xz(), t.yy(), t.yz(), t.zz()};
    std::memcpy(u, c, 48);
    std::printf("%s %d", key.c_str(), int(i));
    for (int k = 0; k < 6; ++k) std::printf(" %016llx", static_cast<unsigned long long>(u[k]));
    std::printf("\n");
}

static void P(const std::string& k, label i, scalar a) { H(k, i, a); }
static void P(const std::string& k, label i, const vector& a) { H3(k, i, a); }
static void P(const std::string& k, label i, const symmTensor& a) { H6(k, i, a); }
static void P(const std::string& k, label i, const tensor& a) { H9(k, i, a); }

// A volume field: cell values under `key`, boundary-face values (patch order) under `key`_b.
template<class T>
static void dumpVF(const std::string& key, const GeometricField<T, fvPatchField, volMesh>& f)
{
    forAll(f.primitiveField(), i) P(key, i, f.primitiveField()[i]);
    label kb = 0;
    forAll(f.boundaryField(), pi)
    {
        forAll(f.boundaryField()[pi], fi) P(key + "_b", kb++, f.boundaryField()[pi][fi]);
    }
}

// ---- analytic inputs: polynomials only (no transcendental functions) --------
// Duct 0<x<4, 0<y<1, 0<z<0.5. s vanishes on the y walls, t on the z walls.
static scalar sOf(const point& p) { return p.y()*(1.0 - p.y()); }
static scalar tOf(const point& p) { return 16.0*p.z()*(0.5 - p.z()); }

static vector fU(const point& p)
{
    const scalar x = p.x();
    const scalar s = sOf(p), t = tOf(p);
    return vector
    (
        4.0*s*t*(1.0 + 0.25*x),
        0.1*s*t*(1.0 + 0.5*x),
        0.05*s*t*x*0.25
    );
}

static scalar fk(const point& p)
{
    return 0.02 + 0.4*sOf(p)*(1.0 + 0.1*p.x()) + 0.01*p.z();
}

static scalar feps(const point& p)
{
    const scalar d = 0.3 - sOf(p);
    return 0.1 + 0.5*d*d + 0.02*p.x();
}

static scalar fomega(const point& p)
{
    const scalar d = 0.25 - sOf(p);
    return 0.5 + 200.0*d*d*(1.0 + 0.1*p.x()) + 5.0*p.x()*p.x() + 4.0*p.z();
}

static scalar fnuTilda(const point& p)
{
    return 5.0e-5 + 4.0e-4*sOf(p)*(1.0 + 0.2*p.x()) + 1.0e-5*p.z();
}

static scalar fzero(const point&) { return 0.0; }

static autoPtr<volScalarField> mkScalar
(
    const fvMesh& mesh,
    const Time& runTime,
    const word& name,
    const dimensionSet& dims,
    scalar (*f)(const point&),
    const word& bcType
)
{
    wordList bt(mesh.boundary().size(), bcType);
    autoPtr<volScalarField> p
    (
        new volScalarField
        (
            IOobject
            (
                name, runTime.timeName(), mesh,
                IOobject::NO_READ, IOobject::NO_WRITE, IOobject::NO_REGISTER
            ),
            mesh, dimensionedScalar(dims, 0), bt
        )
    );
    const vectorField& C = mesh.C().primitiveField();
    forAll(C, c) p->primitiveFieldRef()[c] = f(C[c]);
    p->correctBoundaryConditions();
    p->write();
    return p;
}

// Compare an assembled matrix (as the real model's solver saw it) with the
// transcribed one, bit for bit, and print the count.
static void auditPart
(
    const std::string& eq,
    const char* part,
    const scalarField& real,
    const scalarField& mine
)
{
    label same = 0;
    scalar worst = 0;
    forAll(real, i)
    {
        if (real[i] == mine[i]) ++same;
        else
        {
            const scalar r = mag(real[i] - mine[i])/max(mag(real[i]), VSMALL);
            if (r > worst) worst = r;
        }
    }
    std::printf
    (
        "audit %s %s n=%d identical=%d worst_rel=%.3e\n", eq.c_str(), part,
        int(real.size()), int(same), worst
    );
}

// fvMatrix keeps its boundary-completion helpers protected; expose them.
struct MatrixAccess : public fvScalarMatrix
{
    MatrixAccess(const fvScalarMatrix& m) : fvScalarMatrix(m) {}
    using fvScalarMatrix::addBoundaryDiag;
    using fvScalarMatrix::addBoundarySource;
};

static void auditMatrix(const word& eq, const fvScalarMatrix& mIn)
{
    MatrixAccess m(mIn);
    if (!dumpSolver::captured.found(eq))
    {
        std::printf("audit %s MISSING (real model did not solve it)\n", eq.c_str());
        return;
    }
    const List<scalarField>& real = dumpSolver::captured[eq];
    scalarField d(m.diag());
    m.addBoundaryDiag(d, 0);
    scalarField src(m.source());
    m.addBoundarySource(src, false);
    auditPart(eq, "diag", real[0], d);
    auditPart(eq, "source", real[3], src);
    if (m.hasUpper()) auditPart(eq, "upper", real[1], m.upper());
    if (m.hasLower()) auditPart(eq, "lower", real[2], m.lower());
}

int main(int argc, char* argv[])
{
    argList::addOption("model", "name", "kEpsilon|kOmega|kOmegaSST|SpalartAllmaras|Smagorinsky|RNGkEpsilon|realizableKE|WALE|SpalartAllmarasDES|SpalartAllmarasDDES|SpalartAllmarasIDDES|kOmegaSSTDES|kEqn|LaunderSharmaKE|LESfilters|dynamicKEqn|dynamicLagrangian");
    #include "setRootCase.H"
    #include "createTime.H"
    #include "createMesh.H"
    dimensionSet::checking(false);

    const word model(args.get<word>("model"));
    const vectorField& Cf = mesh.Cf().primitiveField();
    const vectorField& C = mesh.C().primitiveField();
    const label nInt = mesh.nInternalFaces();
    dumpSolver::registry = &mesh;

    // ---- geometry ----------------------------------------------------------
    std::printf("# nCells %d nFaces %d nInternalFaces %d\n",
        int(mesh.nCells()), int(mesh.nFaces()), int(nInt));
    {
        const pointField& pts = mesh.points();
        forAll(pts, i) H3("points", i, pts[i]);
        const faceList& fcs = mesh.faces();
        forAll(fcs, i)
        {
            std::printf("face %d %d", int(i), int(fcs[i].size()));
            forAll(fcs[i], j) std::printf(" %d", int(fcs[i][j]));
            std::printf("\n");
        }
        const labelList& own = mesh.faceOwner();
        const labelList& nei = mesh.faceNeighbour();
        forAll(own, i) std::printf("own %d %d\n", int(i), int(own[i]));
        forAll(nei, i) std::printf("nei %d %d\n", int(i), int(nei[i]));
        forAll(mesh.boundary(), pi)
        {
            std::printf("patch %d %s %d %d\n", int(pi),
                mesh.boundary()[pi].name().c_str(),
                int(mesh.boundary()[pi].start()), int(mesh.boundary()[pi].size()));
        }
        const vectorField& fc = mesh.faceCentres();
        const vectorField& fa = mesh.faceAreas();
        forAll(fc, i)
        {
            H3("Cf", i, fc[i]);
            H3("Sf", i, fa[i]);
        }
        const vectorField& cc = mesh.cellCentres();
        const scalarField& cv = mesh.cellVolumes();
        forAll(cc, i)
        {
            H3("C", i, cc[i]);
            H("V", i, cv[i]);
        }
    }

    // ---- inputs ------------------------------------------------------------
    wordList fixedT(mesh.boundary().size(), fixedValueFvPatchVectorField::typeName);
    volVectorField U
    (
        IOobject("U", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
        mesh, dimensionedVector(dimVelocity, Zero), fixedT
    );
    forAll(C, c) U.primitiveFieldRef()[c] = fU(C[c]);
    forAll(U.boundaryField(), pi)
    {
        vectorField vals(mesh.boundary()[pi].size());
        forAll(vals, fi) vals[fi] = fU(mesh.boundary()[pi].Cf()[fi]);
        U.boundaryFieldRef()[pi] == vals;
    }
    surfaceScalarField phi
    (
        IOobject("phi", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
        mesh, dimensionedScalar(dimVelocity*dimArea, 0)
    );
    forAll(Cf, f)
    {
        if (f < nInt) phi.primitiveFieldRef()[f] = fU(Cf[f]) & mesh.Sf().primitiveField()[f];
    }
    forAll(phi.boundaryField(), pi)
    {
        const fvPatch& p = mesh.boundary()[pi];
        forAll(p, fi) phi.boundaryFieldRef()[pi][fi] = fU(p.Cf()[fi]) & p.Sf()[fi];
    }

    const word zg(zeroGradientFvPatchScalarField::typeName);
    autoPtr<volScalarField> kP = mkScalar(mesh, runTime, "k", dimensionSet(0, 2, -2, 0, 0), fk, zg);
    autoPtr<volScalarField> epsP = mkScalar(mesh, runTime, "epsilon", dimensionSet(0, 2, -3, 0, 0), feps, zg);
    autoPtr<volScalarField> omP = mkScalar(mesh, runTime, "omega", dimensionSet(0, 0, -1, 0, 0), fomega, zg);
    autoPtr<volScalarField> ntP = mkScalar(mesh, runTime, "nuTilda", dimensionSet(0, 2, -1, 0, 0), fnuTilda, zg);
    mkScalar(mesh, runTime, "nut", dimensionSet(0, 2, -1, 0, 0), fzero, word(calculatedFvPatchScalarField::typeName));
    const volScalarField& k = kP();
    const volScalarField& epsilon = epsP();
    const volScalarField& omega = omP();
    const volScalarField& nuTilda = ntP();

    // inputs the Rust side reads back (cell values; boundary values of the
    // zero-gradient scalars equal the owner cell value, so are not dumped)
    dumpV("U", U.primitiveField());
    {
        label kf = 0;
        forAll(U.boundaryField(), pi)
        {
            forAll(U.boundaryField()[pi], fi) H3("Ub", kf++, U.boundaryField()[pi][fi]);
        }
        kf = 0;
        forAll(phi.primitiveField(), f) H("phi", kf++, phi.primitiveField()[f]);
        forAll(phi.boundaryField(), pi)
        {
            forAll(phi.boundaryField()[pi], fi) H("phi", kf++, phi.boundaryField()[pi][fi]);
        }
    }
    dumpS("k", k.primitiveField());
    dumpS("epsilon", epsilon.primitiveField());
    dumpS("omega", omega.primitiveField());
    dumpS("nuTilda", nuTilda.primitiveField());

    // ---- the REAL model ----------------------------------------------------
    singlePhaseTransportModel laminarTransport(U, phi);
    autoPtr<incompressible::turbulenceModel> turbulence
    (
        incompressible::turbulenceModel::New(U, phi, laminarTransport)
    );
    turbulence->validate();
    tmp<volScalarField> tnut = turbulence->nut();
    const volScalarField& nut = tnut();
    tmp<volScalarField> tnu = turbulence->nu();
    const volScalarField& nu = tnu();
    dumpS("nu", nu.primitiveField());
    dumpS("nut_real", nut.primitiveField());
    if (mesh.foundObject<volScalarField>("delta"))
    {
        dumpS("delta_real", mesh.lookupObject<volScalarField>("delta").primitiveField());
    }
    if (mesh.foundObject<volScalarField>("hmax"))
    {
        dumpS("hmax_real", mesh.lookupObject<volScalarField>("hmax").primitiveField());
    }
    {
        const volScalarField& y = wallDist::New(mesh).y();
        dumpS("y", y.primitiveField());
    }

    // frozen gradients as the real code forms them
    tmp<volTensorField> tgradU = fvc::grad(U);
    const volTensorField& gradU = tgradU();
    const volScalarField::Internal divU(fvc::div(fvc::absolute(phi, U))().v());
    dumpT("gradU", gradU.primitiveField());
    {
        label kb = 0;
        forAll(gradU.boundaryField(), pi)
        {
            forAll(gradU.boundaryField()[pi], fi) H9("gradU_b", kb++, gradU.boundaryField()[pi][fi]);
        }
    }
    dumpS("divU", divU.field());
    tmp<volVectorField> tgradk = fvc::grad(k);
    tmp<volVectorField> tgradom = fvc::grad(omega);
    tmp<volVectorField> tgradnt = fvc::grad(nuTilda);
    dumpV("gradk", tgradk().primitiveField());
    dumpV("gradomega", tgradom().primitiveField());
    dumpV("gradnuTilda", tgradnt().primitiveField());

    // ---- the REAL correct(): matrices and local fields are printed by the
    //      dumpSolver instead of being solved ------------------------------
    if (model != "Smagorinsky" && model != "WALE" && model != "LESfilters")
    {
        turbulence->correct();
    }

    // ---- transcribed source terms (see the upstream file named per model) ---
    const dimensionedScalar one("one", dimless, 1.0);
    const volScalarField::Internal oneCell(nut.v()*0 + 1);
    (void)oneCell;

    if (model == "kEpsilon")
    {
        // RASModels/kEpsilon/kEpsilon.C, correct()
        const dimensionedScalar Cmu("Cmu", dimless, 0.09), C1("C1", dimless, 1.44),
            C2("C2", dimless, 1.92), C3("C3", dimless, 0), sigmak("sk", dimless, 1.0),
            sigmaEps("se", dimless, 1.3);
        const volScalarField::Internal GbyNu(gradU.v() && devTwoSymm(gradU.v()));
        const volScalarField::Internal G(nut()*GbyNu);
        dumpS("T_GbyNu", GbyNu.field());
        dumpS("T_G", G.field());
        const volScalarField::Internal Eeps(C1*GbyNu*Cmu*k());
        const volScalarField::Internal SuSpEps(((2.0/3.0)*C1 - C3)*divU);
        const volScalarField::Internal SpEps(C2*epsilon()/k());
        const volScalarField::Internal SuSpK((2.0/3.0)*divU);
        const volScalarField::Internal SpK(epsilon()/k());
        dumpS("T_eps_E", Eeps.field());
        dumpS("T_eps_SuSp", SuSpEps.field());
        dumpS("T_eps_Sp", SpEps.field());
        dumpS("T_k_E", G.field());
        dumpS("T_k_SuSp", SuSpK.field());
        dumpS("T_k_Sp", SpK.field());
        const volScalarField Deps("DepsilonEff", nut/sigmaEps + nu);
        const volScalarField Dk("DkEff", nut/sigmak + nu);
        fvScalarMatrix epsEqn
        (
            fvm::ddt(epsP()) + fvm::div(phi, epsP()) - fvm::laplacian(Deps, epsP())
         ==
            Eeps - fvm::SuSp(SuSpEps, epsP()) - fvm::Sp(SpEps, epsP())
        );
        auditMatrix("epsilon", epsEqn);
        fvScalarMatrix kEqn
        (
            fvm::ddt(kP()) + fvm::div(phi, kP()) - fvm::laplacian(Dk, kP())
         ==
            G - fvm::SuSp(SuSpK, kP()) - fvm::Sp(SpK, kP())
        );
        auditMatrix("k", kEqn);
    }
    else if (model == "kOmega")
    {
        // RASModels/kOmega/kOmega.C, correct()
        const dimensionedScalar Cmu("Cmu", dimless, 0.09), beta("beta", dimless, 0.072),
            gamma("gamma", dimless, 0.52), alphaK("aK", dimless, 0.5),
            alphaOmega("aO", dimless, 0.5);
        const volScalarField::Internal GbyNu(gradU.v() && devTwoSymm(gradU.v()));
        const volScalarField::Internal G(nut()*GbyNu);
        dumpS("T_GbyNu", GbyNu.field());
        dumpS("T_G", G.field());
        const volScalarField::Internal Eom(gamma*GbyNu);
        const volScalarField::Internal SuSpOm(((2.0/3.0)*gamma)*divU);
        const volScalarField::Internal SpOm(beta*omega());
        const volScalarField::Internal SuSpK((2.0/3.0)*divU);
        const volScalarField::Internal SpK(Cmu*omega());
        dumpS("T_omega_E", Eom.field());
        dumpS("T_omega_SuSp", SuSpOm.field());
        dumpS("T_omega_Sp", SpOm.field());
        dumpS("T_k_E", G.field());
        dumpS("T_k_SuSp", SuSpK.field());
        dumpS("T_k_Sp", SpK.field());
        const volScalarField Dom("DomegaEff", alphaOmega*nut + nu);
        const volScalarField Dk("DkEff", alphaK*nut + nu);
        fvScalarMatrix omEqn
        (
            fvm::ddt(omP()) + fvm::div(phi, omP()) - fvm::laplacian(Dom, omP())
         ==
            Eom - fvm::SuSp(SuSpOm, omP()) - fvm::Sp(SpOm, omP())
        );
        auditMatrix("omega", omEqn);
        fvScalarMatrix kEqn
        (
            fvm::ddt(kP()) + fvm::div(phi, kP()) - fvm::laplacian(Dk, kP())
         ==
            G - fvm::SuSp(SuSpK, kP()) - fvm::Sp(SpK, kP())
        );
        auditMatrix("k", kEqn);
    }
    else if (model == "kOmegaSST")
    {
        // Base/kOmegaSST/kOmegaSSTBase.C (v2506 defaults, F3 off, decayControl off)
        const dimensionedScalar alphaK1("aK1", dimless, 0.85), alphaK2("aK2", dimless, 1.0),
            alphaOmega1("aO1", dimless, 0.5), alphaOmega2("aO2", dimless, 0.856),
            gamma1("g1", dimless, 5.0/9.0), gamma2("g2", dimless, 0.44),
            beta1("b1", dimless, 0.075), beta2("b2", dimless, 0.0828),
            betaStar("bs", dimless, 0.09), a1("a1", dimless, 0.31),
            b1("bb1", dimless, 1.0), c1("c1", dimless, 10.0);
        const volScalarField& y = wallDist::New(mesh).y();

        const volScalarField S2(2*magSqr(symm(gradU)));
        volScalarField::Internal GbyNu0(gradU && devTwoSymm(gradU));
        const volScalarField::Internal G(nut*GbyNu0);

        const volScalarField CDkOmega
        (
            (2*alphaOmega2)*(fvc::grad(k) & fvc::grad(omega))/omega
        );
        // F1
        const volScalarField CDkOmegaPlus
        (
            max(CDkOmega, dimensionedScalar(dimless/sqr(dimTime), 1.0e-10))
        );
        const volScalarField arg1
        (
            min
            (
                min
                (
                    max
                    (
                        (scalar(1)/betaStar)*sqrt(k)/(omega*y),
                        scalar(500)*nu/(sqr(y)*omega)
                    ),
                    (4*alphaOmega2)*k/(CDkOmegaPlus*sqr(y))
                ),
                scalar(10)
            )
        );
        const volScalarField F1(tanh(pow4(arg1)));
        // F2 (= F23, F3 off)
        const volScalarField arg2
        (
            min
            (
                max
                (
                    (scalar(2)/betaStar)*sqrt(k)/(omega*y),
                    scalar(500)*nu/(sqr(y)*omega)
                ),
                scalar(100)
            )
        );
        const volScalarField F23(tanh(sqr(arg2)));
        const volScalarField nutT(a1*k/max(a1*omega, b1*F23*sqrt(S2)));

        const volScalarField::Internal gamma(F1()*(gamma1 - gamma2) + gamma2);
        const volScalarField::Internal beta(F1()*(beta1 - beta2) + beta2);
        const volScalarField::Internal Pk(min(G, (c1*betaStar)*k()*omega()));
        const volScalarField::Internal GbyNu
        (
            min(GbyNu0, (c1/a1)*betaStar*omega()*max(a1*omega(), b1*F23()*sqrt(S2())))
        );
        const volScalarField::Internal Eom(gamma*GbyNu);
        const volScalarField::Internal SuSpOm1(((2.0/3.0)*gamma)*divU);
        const volScalarField::Internal SpOm(beta*omega());
        const volScalarField::Internal SuSpOm2((F1() - scalar(1))*CDkOmega()/omega());
        const volScalarField::Internal SuSpK((2.0/3.0)*divU);
        const volScalarField::Internal SpK(betaStar*omega());
        dumpS("T_S2", S2.primitiveField());
        dumpS("T_CDkOmega", CDkOmega.primitiveField());
        dumpS("T_F1", F1.primitiveField());
        dumpS("T_F23", F23.primitiveField());
        dumpS("T_nut", nutT.primitiveField());
        dumpS("T_GbyNu0", GbyNu0.field());
        dumpS("T_G", G.field());
        dumpS("T_gamma", gamma.field());
        dumpS("T_beta", beta.field());
        dumpS("T_GbyNu", GbyNu.field());
        dumpS("T_Pk", Pk.field());
        dumpS("T_omega_E", Eom.field());
        dumpS("T_omega_SuSp1", SuSpOm1.field());
        dumpS("T_omega_Sp", SpOm.field());
        dumpS("T_omega_SuSp2", SuSpOm2.field());
        dumpS("T_k_E", Pk.field());
        dumpS("T_k_SuSp", SuSpK.field());
        dumpS("T_k_Sp", SpK.field());

        const volScalarField alphaKF(F1*(alphaK1 - alphaK2) + alphaK2);
        const volScalarField alphaOF(F1*(alphaOmega1 - alphaOmega2) + alphaOmega2);
        const volScalarField Dk("DkEff", alphaKF*nut + nu);
        const volScalarField Dom("DomegaEff", alphaOF*nut + nu);
        fvScalarMatrix omEqn
        (
            fvm::ddt(omP()) + fvm::div(phi, omP()) - fvm::laplacian(Dom, omP())
         ==
            Eom
          - fvm::SuSp(SuSpOm1, omP())
          - fvm::Sp(SpOm, omP())
          - fvm::SuSp(SuSpOm2, omP())
        );
        auditMatrix("omega", omEqn);
        fvScalarMatrix kEqn
        (
            fvm::ddt(kP()) + fvm::div(phi, kP()) - fvm::laplacian(Dk, kP())
         ==
            Pk - fvm::SuSp(SuSpK, kP()) - fvm::Sp(SpK, kP())
        );
        auditMatrix("k", kEqn);
    }
    else if (model == "SpalartAllmaras")
    {
        // Base/SpalartAllmaras/SpalartAllmarasBase.C, correct()
        const dimensionedScalar sigmaNut("sigmaNut", dimless, 0.66666),
            kappa("kappa", dimless, 0.41), Cb1("Cb1", dimless, 0.1355),
            Cb2("Cb2", dimless, 0.622), Cw2("Cw2", dimless, 0.3),
            Cw3("Cw3", dimless, 2.0), Cv1("Cv1", dimless, 7.1), Cs("Cs", dimless, 0.3);
        const dimensionedScalar Cw1(Cb1/sqr(kappa) + (1.0 + Cb2)/sigmaNut);
        const volScalarField& y = wallDist::New(mesh).y();
        const volScalarField chi(nuTilda/nu);
        const volScalarField chi3("chi3", pow3(chi));
        const volScalarField fv1(chi3/(chi3 + pow3(Cv1)));
        const volScalarField fv2(scalar(1) - chi/(scalar(1) + chi*fv1));
        const volScalarField Omega(std::sqrt(2.0)*mag(skew(gradU)));
        const volScalarField dTilda(y);
        const volScalarField Stilda
        (
            max(Omega + fv2*nuTilda/sqr(kappa*dTilda), Cs*Omega)
        );
        const dimensionedScalar epsS(Stilda.dimensions(), SMALL);
        volScalarField r
        (
            min(nuTilda/(max(Stilda, epsS)*sqr(kappa*dTilda)), scalar(10))
        );
        const volScalarField::Internal rI(r());
        const volScalarField::Internal g(rI + Cw2*(pow6(rI) - rI));
        const volScalarField::Internal fw
        (
            g*pow((1 + pow6(Cw3))/(pow6(g) + pow6(Cw3)), 1.0/6.0)
        );
        const volScalarField::Internal gradSqr(magSqr(fvc::grad(nuTilda)()()));
        const volScalarField::Internal prod(Cb1*Stilda()*nuTilda()*(scalar(1) - scalar(0)));
        const volScalarField::Internal spC
        (
            (Cw1*fw - Cb1/sqr(kappa)*scalar(0))*nuTilda()/sqr(dTilda())
        );
        dumpS("T_chi", chi.primitiveField());
        dumpS("T_fv1", fv1.primitiveField());
        dumpS("T_fv2", fv2.primitiveField());
        dumpS("T_Omega", Omega.primitiveField());
        dumpS("T_Stilda", Stilda.primitiveField());
        dumpS("T_r", rI.field());
        dumpS("T_fw", fw.field());
        dumpS("T_gradSqr", gradSqr.field());
        dumpS("T_nuTilda_E", prod.field());
        dumpS("T_nuTilda_Sp", spC.field());
        const volScalarField nutSA(nuTilda*fv1);
        dumpS("T_nut", nutSA.primitiveField());
        const volScalarField Dnt("DnuTildaEff", (nuTilda + nu)/sigmaNut);
        fvScalarMatrix ntEqn
        (
            fvm::ddt(ntP()) + fvm::div(phi, ntP()) - fvm::laplacian(Dnt, ntP())
          - Cb2/sigmaNut*gradSqr
         ==
            prod - fvm::Sp(spC, ntP())
        );
        auditMatrix("nuTilda", ntEqn);
    }
    else if (model == "RNGkEpsilon")
    {
        // RASModels/RNGkEpsilon/RNGkEpsilon.C, correct()
        const dimensionedScalar Cmu("Cmu", dimless, 0.0845), C1("C1", dimless, 1.42),
            C2("C2", dimless, 1.68), C3("C3", dimless, 0), sigmak("sk", dimless, 0.71942),
            sigmaEps("se", dimless, 0.71942), eta0("eta0", dimless, 4.38),
            beta("beta", dimless, 0.012);
        const volScalarField::Internal GbyNu(gradU.v() && devTwoSymm(gradU.v()));
        const volScalarField::Internal G(nut()*GbyNu);
        const volScalarField::Internal eta(sqrt(mag(GbyNu))*k()/epsilon());
        const volScalarField::Internal eta3(eta*sqr(eta));
        const volScalarField::Internal R
        (
            ((eta*(-eta/eta0 + scalar(1)))/(beta*eta3 + scalar(1)))
        );
        dumpS("T_GbyNu", GbyNu.field());
        dumpS("T_G", G.field());
        dumpS("T_eta", eta.field());
        dumpS("T_R", R.field());
        const volScalarField::Internal Eeps((C1 - R)*GbyNu*Cmu*k());
        const volScalarField::Internal SuSpEps(((2.0/3.0)*C1 - C3)*divU);
        const volScalarField::Internal SpEps(C2*epsilon()/k());
        const volScalarField::Internal SuSpK((2.0/3.0)*divU);
        const volScalarField::Internal SpK(epsilon()/k());
        dumpS("T_eps_E", Eeps.field());
        dumpS("T_eps_SuSp", SuSpEps.field());
        dumpS("T_eps_Sp", SpEps.field());
        dumpS("T_k_E", G.field());
        dumpS("T_k_SuSp", SuSpK.field());
        dumpS("T_k_Sp", SpK.field());
        const volScalarField nutT(Cmu*sqr(k)/epsilon);
        dumpS("T_nut", nutT.primitiveField());
        const volScalarField Deps("DepsilonEff", nut/sigmaEps + nu);
        const volScalarField Dk("DkEff", nut/sigmak + nu);
        fvScalarMatrix epsEqn
        (
            fvm::ddt(epsP()) + fvm::div(phi, epsP()) - fvm::laplacian(Deps, epsP())
         ==
            Eeps - fvm::SuSp(SuSpEps, epsP()) - fvm::Sp(SpEps, epsP())
        );
        auditMatrix("epsilon", epsEqn);
        fvScalarMatrix kEqn
        (
            fvm::ddt(kP()) + fvm::div(phi, kP()) - fvm::laplacian(Dk, kP())
         ==
            G - fvm::SuSp(SuSpK, kP()) - fvm::Sp(SpK, kP())
        );
        auditMatrix("k", kEqn);
    }
    else if (model == "realizableKE")
    {
        // RASModels/realizableKE/realizableKE.C, rCmu() and correct()
        const dimensionedScalar A0("A0", dimless, 4.0), C2("C2", dimless, 1.9),
            sigmak("sk", dimless, 1.0), sigmaEps("se", dimless, 1.2);
        const volScalarField S2(2*magSqr(devSymm(gradU)));
        const volScalarField magS(sqrt(S2));
        tmp<volSymmTensorField> tS = devSymm(gradU);
        const volSymmTensorField& S = tS();
        const volScalarField W
        (
            (2*Foam::sqrt(2.0))*((S&S)&&S)
           /(
                magS*S2
              + dimensionedScalar("small", dimensionSet(0, 0, -3, 0, 0), SMALL)
            )
        );
        const volScalarField phis
        (
            (1.0/3.0)*acos(clamp(Foam::sqrt(6.0)*W, scalarMinMax(-1, 1)))
        );
        const volScalarField As(Foam::sqrt(6.0)*cos(phis));
        const volScalarField Us(sqrt(S2/2.0 + magSqr(skew(gradU))));
        const volScalarField rCmu(1.0/(A0 + As*Us*k/epsilon));
        const volScalarField nutT(rCmu*sqr(k)/epsilon);
        const volScalarField eta(magS*k/epsilon);
        const volScalarField C1(max(eta/(scalar(5) + eta), scalar(0.43)));
        const volScalarField G(nut*(gradU && devTwoSymm(gradU)));
        const volScalarField nuL(max(nu, dimensionedScalar(nu.dimensions(), Zero)));
        const volScalarField::Internal Eeps(C1()*magS()*epsilon());
        const volScalarField::Internal SpEps(C2*epsilon()/(k() + sqrt(nuL()*epsilon())));
        const volScalarField::Internal SuSpK((2.0/3.0)*divU);
        const volScalarField::Internal SpK(epsilon()/k());
        dumpS("T_S2", S2.primitiveField());
        dumpS("T_magS", magS.primitiveField());
        dumpS("T_W", W.primitiveField());
        dumpS("T_phis", phis.primitiveField());
        dumpS("T_As", As.primitiveField());
        dumpS("T_Us", Us.primitiveField());
        dumpS("T_rCmu", rCmu.primitiveField());
        dumpS("T_nut", nutT.primitiveField());
        dumpS("T_eta", eta.primitiveField());
        dumpS("T_C1", C1.primitiveField());
        dumpS("T_G", G.primitiveField());
        dumpS("T_eps_E", Eeps.field());
        dumpS("T_eps_Sp", SpEps.field());
        dumpS("T_k_E", G.primitiveField());
        dumpS("T_k_SuSp", SuSpK.field());
        dumpS("T_k_Sp", SpK.field());
        const volScalarField Deps("DepsilonEff", nut/sigmaEps + nu);
        const volScalarField Dk("DkEff", nut/sigmak + nu);
        fvScalarMatrix epsEqn
        (
            fvm::ddt(epsP()) + fvm::div(phi, epsP()) - fvm::laplacian(Deps, epsP())
         ==
            Eeps - fvm::Sp(SpEps, epsP())
        );
        auditMatrix("epsilon", epsEqn);
        fvScalarMatrix kEqn
        (
            fvm::ddt(kP()) + fvm::div(phi, kP()) - fvm::laplacian(Dk, kP())
         ==
            G() - fvm::SuSp(SuSpK, kP()) - fvm::Sp(SpK, kP())
        );
        auditMatrix("k", kEqn);
    }
    else if (model == "WALE")
    {
        // LESModels/WALE/WALE.C k(), correctNut(); delta cubeRootVol (deltaCoeff 1)
        const dimensionedScalar Ck("Ck", dimless, 0.094), Cw("Cw", dimless, 0.325);
        volScalarField delta
        (
            IOobject("delta", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE, IOobject::NO_REGISTER),
            mesh, dimensionedScalar(dimLength, 0), zeroGradientFvPatchScalarField::typeName
        );
        delta.primitiveFieldRef() = 1.0*cbrt(mesh.V());
        delta.correctBoundaryConditions();
        const volSymmTensorField Sd(devSymm(gradU & gradU));
        const volScalarField magSqrSd(magSqr(Sd));
        const volScalarField magSqrS(magSqr(symm(gradU)));
        const volScalarField pw1(pow(magSqrS, 5.0/2.0));
        const volScalarField pw2(pow(magSqrSd, 5.0/4.0));
        const volScalarField kk
        (
            sqr(sqr(Cw)*delta/Ck)*
            (
                pow3(magSqrSd)
               /(
                   sqr(pw1 + pw2)
                 + dimensionedScalar("SMALL", dimensionSet(0, 0, -10, 0, 0), SMALL)
               )
            )
        );
        const volScalarField nutT(Ck*delta*sqrt(kk));
        dumpS("T_delta", delta.primitiveField());
        dumpS("T_magSqrSd", magSqrSd.primitiveField());
        dumpS("T_magSqrS", magSqrS.primitiveField());
        dumpS("T_pow52", pw1.primitiveField());
        dumpS("T_pow54", pw2.primitiveField());
        dumpS("T_k", kk.primitiveField());
        dumpS("T_nut", nutT.primitiveField());
    }
    else if
    (
        model == "SpalartAllmarasDES" || model == "SpalartAllmarasDDES"
     || model == "SpalartAllmarasIDDES"
    )
    {
        // DES/SpalartAllmarasDES, SpalartAllmarasDDES, SpalartAllmarasIDDES on
        // Base/SpalartAllmaras/SpalartAllmarasBase.C (useSigma false, ft2 off,
        // lowReCorrection on, standard shielding, fe on)
        const bool ddes = (model == "SpalartAllmarasDDES");
        const bool iddes = (model == "SpalartAllmarasIDDES");
        const dimensionedScalar sigmaNut("sigmaNut", dimless, 0.66666),
            kappa("kappa", dimless, 0.41), Cb1("Cb1", dimless, 0.1355),
            Cb2("Cb2", dimless, 0.622), Cw2("Cw2", dimless, 0.3),
            Cw3("Cw3", dimless, 2.0), Cv1("Cv1", dimless, 7.1), Cs("Cs", dimless, 0.3),
            CDES("CDES", dimless, 0.65), fwStar("fwStar", dimless, 0.424),
            Cd1("Cd1", dimless, 8), Cd2("Cd2", dimless, 3),
            Cdt1("Cdt1", dimless, 8), Cdt2("Cdt2", dimless, 3),
            Cl("Cl", dimless, 3.55), Ct("Ct", dimless, 1.63);
        const dimensionedScalar Cw1(Cb1/sqr(kappa) + (1.0 + Cb2)/sigmaNut);
        const volScalarField& y = wallDist::New(mesh).y();
        const volScalarField& delta = mesh.lookupObject<volScalarField>("delta");
        const volScalarField ft2
        (
            IOobject("ft2", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE, IOobject::NO_REGISTER),
            mesh, dimensionedScalar(dimless, 0)
        );
        const volScalarField chi(nuTilda/nu);
        const volScalarField chi3("chi3", pow3(chi));
        const volScalarField fv1(chi3/(chi3 + pow3(Cv1)));
        const volScalarField fv2(scalar(1) - chi/(scalar(1) + chi*fv1));
        const volScalarField psi
        (
            sqrt
            (
                min
                (
                    scalar(100),
                    (1 - Cb1/(Cw1*sqr(kappa)*fwStar)*(ft2 + (1 - ft2)*fv2))
                   /max(SMALL, (fv1*max(scalar(1e-10), 1 - ft2)))
                )
            )
        );
        const volScalarField lLES(psi*CDES*delta);
        const volScalarField magGradU(mag(gradU));
        const dimensionedScalar epsG(dimless, SMALL);
        auto rOf = [&](const volScalarField& nur)
        {
            return volScalarField
            (
                min(nur/(max(magGradU, epsG)*sqr(kappa*y)), scalar(10))
            );
        };
        const volScalarField nuEff(nut + nu);
        const dimensionedScalar l0(dimLength, Zero);
        const dimensionedScalar small("small", dimLength, SMALL);
        volScalarField dTilda(lLES);
        volScalarField fdOut(0*lLES);
        if (ddes)
        {
            const volScalarField r(rOf(nuEff));
            const volScalarField fd(1 - tanh(pow(Cd1*r, Cd2)));
            fdOut = fd;
            dTilda = max(y - fd*max(y - lLES, l0), small);
            dumpS("T_r_fd", r.primitiveField());
        }
        else if (iddes)
        {
            const volScalarField& hmax = mesh.lookupObject<volScalarField>("hmax");
            const volScalarField alpha(max(0.25 - y/hmax, scalar(-5)));
            const volScalarField expTerm(exp(sqr(alpha)));
            const volScalarField fB(min(2*pow(expTerm, -9.0), scalar(1)));
            const volScalarField rt(rOf(nut));
            const volScalarField rl(rOf(nu));
            const volScalarField fdt(1 - tanh(pow(Cdt1*rt, Cdt2)));
            const volScalarField fdTilda(max(1 - fdt, fB));
            const volScalarField ft(tanh(pow3(sqr(Ct)*rt)));
            const volScalarField fl(tanh(pow(sqr(Cl)*rl, 10)));
            const volScalarField fe1(2*lerp(pow(expTerm, -9.), pow(expTerm, -11.09), pos0(alpha)));
            const volScalarField fe2(1 - max(ft, fl));
            const volScalarField fe(max(fe1 - 1, scalar(0))*psi*fe2);
            fdOut = fdTilda;
            dTilda = max(fdTilda*(1 + fe)*y + (1 - fdTilda)*lLES, small);
            dumpS("T_alpha", alpha.primitiveField());
            dumpS("T_fB", fB.primitiveField());
            dumpS("T_fdt", fdt.primitiveField());
            dumpS("T_ft", ft.primitiveField());
            dumpS("T_fl", fl.primitiveField());
            dumpS("T_fe1", fe1.primitiveField());
            dumpS("T_fe", fe.primitiveField());
        }
        else
        {
            dTilda = min(lLES, y);
        }
        const volScalarField Omega(std::sqrt(2.0)*mag(skew(gradU)));
        const volScalarField Stilda
        (
            max(Omega + fv2*nuTilda/sqr(kappa*dTilda), Cs*Omega)
        );
        const dimensionedScalar epsS(Stilda.dimensions(), SMALL);
        volScalarField r
        (
            min(nuTilda/(max(Stilda, epsS)*sqr(kappa*dTilda)), scalar(10))
        );
        const volScalarField::Internal rI(r());
        const volScalarField::Internal g(rI + Cw2*(pow6(rI) - rI));
        const volScalarField::Internal fw
        (
            g*pow((1 + pow6(Cw3))/(pow6(g) + pow6(Cw3)), 1.0/6.0)
        );
        const volScalarField::Internal gradSqr(magSqr(fvc::grad(nuTilda)()()));
        const volScalarField::Internal prod(Cb1*Stilda()*nuTilda()*(scalar(1) - scalar(0)));
        const volScalarField::Internal spC
        (
            (Cw1*fw - Cb1/sqr(kappa)*scalar(0))*nuTilda()/sqr(dTilda())
        );
        const volScalarField LESRegion(neg(dTilda - y));
        dumpS("T_delta", delta.primitiveField());
        dumpS("T_chi", chi.primitiveField());
        dumpS("T_fv1", fv1.primitiveField());
        dumpS("T_fv2", fv2.primitiveField());
        dumpS("T_psi", psi.primitiveField());
        dumpS("T_lLES", lLES.primitiveField());
        dumpS("T_fd", fdOut.primitiveField());
        dumpS("T_dTilda", dTilda.primitiveField());
        dumpS("T_Omega", Omega.primitiveField());
        dumpS("T_Stilda", Stilda.primitiveField());
        dumpS("T_r", rI.field());
        dumpS("T_fw", fw.field());
        dumpS("T_gradSqr", gradSqr.field());
        dumpS("T_nuTilda_E", prod.field());
        dumpS("T_nuTilda_Sp", spC.field());
        dumpS("T_LESRegion", LESRegion.primitiveField());
        const volScalarField nutSA(nuTilda*fv1);
        dumpS("T_nut", nutSA.primitiveField());
        const volScalarField Dnt("DnuTildaEff", (nuTilda + nu)/sigmaNut);
        fvScalarMatrix ntEqn
        (
            fvm::ddt(ntP()) + fvm::div(phi, ntP()) - fvm::laplacian(Dnt, ntP())
          - Cb2/sigmaNut*gradSqr
         ==
            prod - fvm::Sp(spC, ntP())
        );
        auditMatrix("nuTilda", ntEqn);
    }
    else if (model == "kOmegaSSTDES")
    {
        // DES/kOmegaSSTDES on Base/kOmegaSST/kOmegaSSTBase.C (useSigma false)
        const dimensionedScalar alphaK1("aK1", dimless, 0.85), alphaK2("aK2", dimless, 1.0),
            alphaOmega1("aO1", dimless, 0.5), alphaOmega2("aO2", dimless, 0.856),
            gamma1("g1", dimless, 5.0/9.0), gamma2("g2", dimless, 0.44),
            beta1("b1", dimless, 0.075), beta2("b2", dimless, 0.0828),
            betaStar("bs", dimless, 0.09), a1("a1", dimless, 0.31),
            b1("bb1", dimless, 1.0), c1("c1", dimless, 10.0),
            CDESkom("CDESkom", dimless, 0.82), CDESkeps("CDESkeps", dimless, 0.60);
        const volScalarField& y = wallDist::New(mesh).y();
        const volScalarField& delta = mesh.lookupObject<volScalarField>("delta");

        const volScalarField S2(2*magSqr(symm(gradU)));
        volScalarField::Internal GbyNu0(gradU && devTwoSymm(gradU));
        const volScalarField::Internal G(nut*GbyNu0);
        const volScalarField CDkOmega
        (
            (2*alphaOmega2)*(fvc::grad(k) & fvc::grad(omega))/omega
        );
        const volScalarField CDkOmegaPlus
        (
            max(CDkOmega, dimensionedScalar(dimless/sqr(dimTime), 1.0e-10))
        );
        const volScalarField arg1
        (
            min
            (
                min
                (
                    max
                    (
                        (scalar(1)/betaStar)*sqrt(k)/(omega*y),
                        scalar(500)*nu/(sqr(y)*omega)
                    ),
                    (4*alphaOmega2)*k/(CDkOmegaPlus*sqr(y))
                ),
                scalar(10)
            )
        );
        const volScalarField F1(tanh(pow4(arg1)));
        const volScalarField arg2
        (
            min
            (
                max
                (
                    (scalar(2)/betaStar)*sqrt(k)/(omega*y),
                    scalar(500)*nu/(sqr(y)*omega)
                ),
                scalar(100)
            )
        );
        const volScalarField F23(tanh(sqr(arg2)));
        const volScalarField nutT(a1*k/max(a1*omega, b1*F23*sqrt(S2)));

        const volScalarField CDES(F1*(CDESkom - CDESkeps) + CDESkeps);
        const volScalarField lRAS(sqrt(k)/(betaStar*omega));
        const volScalarField lLES(CDES*delta);
        const volScalarField dTilda(min(lLES, lRAS));
        const volScalarField::Internal epsByk(sqrt(k())/dTilda());
        const volScalarField LESRegion(neg(dTilda - lRAS));

        const volScalarField::Internal gamma(F1()*(gamma1 - gamma2) + gamma2);
        const volScalarField::Internal beta(F1()*(beta1 - beta2) + beta2);
        const volScalarField::Internal Pk(min(G, (c1*betaStar)*k()*omega()));
        const volScalarField::Internal GbyNu(GbyNu0);
        const volScalarField::Internal Eom(gamma*GbyNu);
        const volScalarField::Internal SuSpOm1(((2.0/3.0)*gamma)*divU);
        const volScalarField::Internal SpOm(beta*omega());
        const volScalarField::Internal SuSpOm2((F1() - scalar(1))*CDkOmega()/omega());
        const volScalarField::Internal SuSpK((2.0/3.0)*divU);
        dumpS("T_delta", delta.primitiveField());
        dumpS("T_S2", S2.primitiveField());
        dumpS("T_CDkOmega", CDkOmega.primitiveField());
        dumpS("T_F1", F1.primitiveField());
        dumpS("T_F23", F23.primitiveField());
        dumpS("T_nut", nutT.primitiveField());
        dumpS("T_CDES", CDES.primitiveField());
        dumpS("T_lRAS", lRAS.primitiveField());
        dumpS("T_lLES", lLES.primitiveField());
        dumpS("T_dTilda", dTilda.primitiveField());
        dumpS("T_epsByk", epsByk.field());
        dumpS("T_LESRegion", LESRegion.primitiveField());
        dumpS("T_GbyNu0", GbyNu0.field());
        dumpS("T_G", G.field());
        dumpS("T_gamma", gamma.field());
        dumpS("T_beta", beta.field());
        dumpS("T_GbyNu", GbyNu.field());
        dumpS("T_Pk", Pk.field());
        dumpS("T_omega_E", Eom.field());
        dumpS("T_omega_SuSp1", SuSpOm1.field());
        dumpS("T_omega_Sp", SpOm.field());
        dumpS("T_omega_SuSp2", SuSpOm2.field());
        dumpS("T_k_E", Pk.field());
        dumpS("T_k_SuSp", SuSpK.field());
        dumpS("T_k_Sp", epsByk.field());

        const volScalarField alphaKF(F1*(alphaK1 - alphaK2) + alphaK2);
        const volScalarField alphaOF(F1*(alphaOmega1 - alphaOmega2) + alphaOmega2);
        const volScalarField Dk("DkEff", alphaKF*nut + nu);
        const volScalarField Dom("DomegaEff", alphaOF*nut + nu);
        fvScalarMatrix omEqn
        (
            fvm::ddt(omP()) + fvm::div(phi, omP()) - fvm::laplacian(Dom, omP())
         ==
            Eom
          - fvm::SuSp(SuSpOm1, omP())
          - fvm::Sp(SpOm, omP())
          - fvm::SuSp(SuSpOm2, omP())
        );
        auditMatrix("omega", omEqn);
        fvScalarMatrix kEqn
        (
            fvm::ddt(kP()) + fvm::div(phi, kP()) - fvm::laplacian(Dk, kP())
         ==
            Pk - fvm::SuSp(SuSpK, kP()) - fvm::Sp(epsByk, kP())
        );
        auditMatrix("k", kEqn);
    }
    else if (model == "kEqn")
    {
        // LESModels/kEqn/kEqn.C correct(), correctNut(); delta cubeRootVol; Ce from LESModel
        const dimensionedScalar Ck("Ck", dimless, 0.094), Ce("Ce", dimless, 1.048);
        const volScalarField& delta = mesh.lookupObject<volScalarField>("delta");
        const volScalarField nutT(Ck*sqrt(k)*delta);
        const volScalarField GbyNu(gradU && devTwoSymm(gradU));
        const volScalarField G(nut*GbyNu);
        const volScalarField::Internal SuSpK((2.0/3.0)*divU);
        const volScalarField SpK(Ce*sqrt(k)/delta);
        dumpS("T_delta", delta.primitiveField());
        dumpS("T_nut", nutT.primitiveField());
        dumpS("T_GbyNu", GbyNu.primitiveField());
        dumpS("T_G", G.primitiveField());
        dumpS("T_k_SuSp", SuSpK.field());
        dumpS("T_k_Sp", SpK.primitiveField());
        const volScalarField Dk("DkEff", nut + nu);
        fvScalarMatrix kEqn
        (
            fvm::ddt(kP()) + fvm::div(phi, kP()) - fvm::laplacian(Dk, kP())
         ==
            G - fvm::SuSp(SuSpK, kP()) - fvm::Sp(SpK, kP())
        );
        auditMatrix("k", kEqn);
    }
    else if (model == "LaunderSharmaKE")
    {
        // RASModels/LaunderSharmaKE/LaunderSharmaKE.C, fMu(), f2(), correct()
        const dimensionedScalar Cmu("Cmu", dimless, 0.09), C1("C1", dimless, 1.44),
            C2("C2", dimless, 1.92), C3("C3", dimless, 0), sigmak("sk", dimless, 1.0),
            sigmaEps("se", dimless, 1.3);
        const volScalarField fMu(exp(-3.4/sqr(scalar(1) + sqr(k)/(nu*epsilon)/50.0)));
        const volScalarField f2(scalar(1) - 0.3*exp(-min(sqr(sqr(k)/(nu*epsilon)), scalar(50))));
        const volScalarField nutT(Cmu*fMu*sqr(k)/epsilon);
        const volScalarField E(2.0*nu*nut*fvc::magSqrGradGrad(U));
        const volScalarField D(2.0*nu*magSqr(fvc::grad(sqrt(k))));
        const volScalarField G(nut*(gradU && devTwoSymm(gradU)));
        const volScalarField Eeps1(C1*G*epsilon/k);
        const volScalarField::Internal SuSpEps(((2.0/3.0)*C1 - C3)*divU);
        const volScalarField SpEps(C2*f2*epsilon/k);
        const volScalarField::Internal SuSpK(2.0/3.0*divU);
        const volScalarField SpK((epsilon + D)/k);
        dumpS("T_fMu", fMu.primitiveField());
        dumpS("T_f2", f2.primitiveField());
        dumpS("T_nut", nutT.primitiveField());
        dumpS("T_E", E.primitiveField());
        dumpS("T_D", D.primitiveField());
        dumpS("T_G", G.primitiveField());
        dumpS("T_eps_E1", Eeps1.primitiveField());
        dumpS("T_eps_SuSp", SuSpEps.field());
        dumpS("T_eps_Sp", SpEps.primitiveField());
        dumpS("T_k_E", G.primitiveField());
        dumpS("T_k_SuSp", SuSpK.field());
        dumpS("T_k_Sp", SpK.primitiveField());
        dumpS("magSqrGradGradU", fvc::magSqrGradGrad(U)().primitiveField());
        dumpV("gradSqrtk", fvc::grad(sqrt(k))().primitiveField());
        const volScalarField Deps("DepsilonEff", nut/sigmaEps + nu);
        const volScalarField Dk("DkEff", nut/sigmak + nu);
        fvScalarMatrix epsEqn
        (
            fvm::ddt(epsP()) + fvm::div(phi, epsP()) - fvm::laplacian(Deps, epsP())
         ==
            Eeps1 - fvm::SuSp(SuSpEps, epsP()) - fvm::Sp(SpEps, epsP()) + E
        );
        auditMatrix("epsilon", epsEqn);
        fvScalarMatrix kEqn
        (
            fvm::ddt(kP()) + fvm::div(phi, kP()) - fvm::laplacian(Dk, kP())
         ==
            G - fvm::SuSp(SuSpK, kP()) - fvm::Sp(SpK, kP())
        );
        auditMatrix("k", kEqn);
    }
    else if (model == "LESfilters")
    {
        // LESfilters/{simpleFilter,laplaceFilter,anisotropicFilter}: the REAL filter classes
        // applied to scalar (k, zeroGradient; magSqr(U), calculated), vector (U, fixedValue),
        // symmTensor (sqr(U), symm(gradU), calculated) and tensor (gradU) fields.
        const scalar widthCoeff = 2.0;
        simpleFilter sf(mesh);
        laplaceFilter lf(mesh, widthCoeff);
        anisotropicFilter af(mesh, widthCoeff);
        // coefficient fields transcribed from the constructors (the classes keep them private)
        {
            const scalarField lc(pow(mesh.V().field(), 2.0/3.0)/widthCoeff);
            dumpS("T_laplaceCoeff", lc);
            vectorField ac(mesh.nCells());
            for (direction d = 0; d < vector::nComponents; d++)
            {
                ac.replace
                (
                    d,
                    (1/widthCoeff)*
                    sqr
                    (
                        2.0*mesh.V().field()
                       /fvc::surfaceSum(mag(mesh.Sf().component(d)))().primitiveField()
                    )
                );
            }
            dumpV("T_anisoCoeff", ac);
        }
        const volScalarField magSqrU(magSqr(U));
        const volSymmTensorField sqrU(sqr(U));
        const volSymmTensorField Dsym(symm(gradU));
        const volTensorField gU(gradU);
        dumpVF("in_magSqrU", magSqrU);
        dumpVF("in_sqrU", sqrU);
        dumpVF("in_Dsym", Dsym);
        {
            tmp<volScalarField> a = sf(k);
            dumpVF("F_simple_k", a());
            tmp<volScalarField> a2 = sf(magSqrU);
            dumpVF("F_simple_magSqrU", a2());
            tmp<volVectorField> b = sf(U);
            dumpVF("F_simple_U", b());
            tmp<volSymmTensorField> c = sf(sqrU);
            dumpVF("F_simple_sqrU", c());
            tmp<volSymmTensorField> c2 = sf(Dsym);
            dumpVF("F_simple_Dsym", c2());
            tmp<volTensorField> d = sf(gU);
            dumpVF("F_simple_gradU", d());
        }
        {
            tmp<volScalarField> a = lf(k);
            dumpVF("F_laplace_k", a());
            tmp<volScalarField> a2 = lf(magSqrU);
            dumpVF("F_laplace_magSqrU", a2());
            tmp<volVectorField> b = lf(U);
            dumpVF("F_laplace_U", b());
            tmp<volSymmTensorField> c = lf(sqrU);
            dumpVF("F_laplace_sqrU", c());
            tmp<volSymmTensorField> c2 = lf(Dsym);
            dumpVF("F_laplace_Dsym", c2());
            tmp<volTensorField> d = lf(gU);
            dumpVF("F_laplace_gradU", d());
        }
        {
            tmp<volScalarField> a = af(k);
            dumpVF("F_aniso_k", a());
            tmp<volScalarField> a2 = af(magSqrU);
            dumpVF("F_aniso_magSqrU", a2());
            tmp<volVectorField> b = af(U);
            dumpVF("F_aniso_U", b());
            tmp<volSymmTensorField> c = af(sqrU);
            dumpVF("F_aniso_sqrU", c());
            tmp<volSymmTensorField> c2 = af(Dsym);
            dumpVF("F_aniso_Dsym", c2());
            tmp<volTensorField> d = af(gU);
            dumpVF("F_aniso_gradU", d());
        }
    }
    else if (model == "Smagorinsky")
    {
        // LESModels/Smagorinsky/Smagorinsky.C k(), correctNut(); delta from
        // LESdeltas/cubeRootVolDelta (deltaCoeff 1)
        const dimensionedScalar Ck("Ck", dimless, 0.094), Ce("Ce", dimless, 1.048);
        volScalarField delta
        (
            IOobject("delta", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE, IOobject::NO_REGISTER),
            mesh, dimensionedScalar(dimLength, 0), zeroGradientFvPatchScalarField::typeName
        );
        delta.primitiveFieldRef() = 1.0*cbrt(mesh.V());
        delta.correctBoundaryConditions();
        const volSymmTensorField D(symm(gradU));
        const volScalarField a(Ce/delta);
        const volScalarField b((2.0/3.0)*tr(D));
        const volScalarField c(2*Ck*delta*(dev(D) && D));
        const volScalarField kk(sqr((-b + sqrt(sqr(b) + 4*a*c))/(2*a)));
        const volScalarField nutT(Ck*delta*sqrt(kk));
        dumpS("T_delta", delta.primitiveField());
        dumpS("T_k", kk.primitiveField());
        dumpS("T_nut", nutT.primitiveField());
    }

    return 0;
}
