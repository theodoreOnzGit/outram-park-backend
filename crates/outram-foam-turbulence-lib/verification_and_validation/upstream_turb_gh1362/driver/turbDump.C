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
#include "dynamicKEqn.H"
#include <cstdio>
#include <cstring>
#include <cstdint>
#include <string>

using namespace Foam;

// The REAL protected Ck(D, KK) and Ce(D, KK) of dynamicKEqn, reached through a using-declaration
// (the symbols are exported by libincompressibleTurbulenceModels).
typedef LESModels::dynamicKEqn<IncompressibleTurbulenceModel<transportModel>> RealDynK;
struct RealDynKAccess : public RealDynK
{
    using RealDynK::Ck;
    using RealDynK::Ce;
};

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

static scalar fflm(const point& p)
{
    return 0.02 + 0.3*sOf(p)*(1.0 + 0.1*p.x()) + 0.01*p.z();
}

static scalar ffmm(const point& p)
{
    return 0.05 + 0.2*sOf(p)*(1.0 + 0.05*p.x()) + 0.02*p.z();
}

static scalar fzero(const point&) { return 0.0; }

static volScalarField::Internal mkInt(const fvMesh& mesh, const Time& runTime, const scalarField& f)
{
    return volScalarField::Internal
    (
        IOobject("tmpInt", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE, IOobject::NO_REGISTER),
        mesh, dimless, f
    );
}

// kOmegaSSTLM only (README slice 8): fields that put the transition model's branches in play.
static scalar fkLM(const point& p)
{
    return 2.0e-5 + 3.0e-4*sOf(p)*(1.0 + 0.1*p.x()) + 2.0e-5*p.z();
}
static scalar fReThetat(const point& p)
{
    return 120.0 + 6400.0*sOf(p)*(1.0 + 0.1*p.x()) + 400.0*p.z();
}
static scalar fgammaInt(const point& p)
{
    return 0.01 + 3.0*sOf(p)*(1.0 + 0.05*p.x()) + 0.05*p.z();
}
static scalar fgammaIntEff(const point& p)
{
    return 0.02 + 24.0*sOf(p)*sOf(p)*(1.0 + 0.1*p.x()) + 0.1*p.z();
}

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
    dumpS("TM_" + std::string(eq.c_str()) + "_diag", d);
    auditPart(eq, "diag", real[0], d);
    auditPart(eq, "source", real[3], src);
    if (m.hasUpper()) auditPart(eq, "upper", real[1], m.upper());
    if (m.hasLower()) auditPart(eq, "lower", real[2], m.lower());
}


// dynamicKEqn.C, transcribed expression for expression (Ck(D, KK), Ce(D, KK)).
static volScalarField dynCk
(
    const simpleFilter& sF, const LESfilter& F, const volVectorField& U,
    const volScalarField& delta, const volSymmTensorField& D, const volScalarField& KK,
    autoPtr<volSymmTensorField>& LLout, autoPtr<volSymmTensorField>& MMout
)
{
    const volSymmTensorField LL
    (
        sF(dev(F(sqr(U)) - (sqr(F(U)))))
    );
    const volSymmTensorField MM
    (
        sF
        (
            -2.0*delta*sqrt
            (
                max(KK, dimensionedScalar(KK.dimensions(), Zero))
            )*F(D)
        )
    );
    const volScalarField Ck
    (
        sF(0.5*(LL && MM))
       /(
            sF(magSqr(MM))
          + dimensionedScalar("small", sqr(MM.dimensions()), VSMALL)
        )
    );
    LLout.reset(new volSymmTensorField("LL", LL));
    MMout.reset(new volSymmTensorField("MM", MM));
    tmp<volScalarField> tfld = 0.5*(mag(Ck) + Ck);
    return tfld();
}

static volScalarField dynCe
(
    const simpleFilter& sF, const LESfilter& F, const volScalarField& nut,
    const volScalarField& nu, const volScalarField& delta, const volSymmTensorField& D,
    const volScalarField& KK
)
{
    const volScalarField Ce
    (
        sF(volScalarField::New("nuEff", nut + nu)*(F(magSqr(D)) - magSqr(F(D))))
       /sF(pow(KK, 1.5)/(2.0*delta))
    );
    tmp<volScalarField> tfld = 0.5*(mag(Ce) + Ce);
    return tfld();
}

int main(int argc, char* argv[])
{
    argList::addOption("model", "name", "kEpsilon|kOmega|kOmegaSST|SpalartAllmaras|Smagorinsky|RNGkEpsilon|realizableKE|WALE|SpalartAllmarasDES|SpalartAllmarasDDES|SpalartAllmarasIDDES|kOmegaSSTDES|kEqn|LaunderSharmaKE|LESfilters|dynamicKEqn|dynamicLagrangian|kOmegaSSTLM");
    argList::addOption("filter", "name", "simple|laplace|anisotropic (dynamicKEqn test filter)");
    #include "setRootCase.H"
    #include "createTime.H"
    #include "createMesh.H"
    dimensionSet::checking(false);

    const word model(args.get<word>("model"));
    const word filterName(args.getOrDefault<word>("filter", "simple"));
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
    const bool isLM = (model == "kOmegaSSTLM");
    autoPtr<volScalarField> kP = mkScalar(mesh, runTime, "k", dimensionSet(0, 2, -2, 0, 0), isLM ? fkLM : fk, zg);
    autoPtr<volScalarField> reThetatP, gammaIntP, gEffInP;
    if (isLM)
    {
        reThetatP = mkScalar(mesh, runTime, "ReThetat", dimless, fReThetat, zg);
        gammaIntP = mkScalar(mesh, runTime, "gammaInt", dimless, fgammaInt, zg);
        gEffInP = mkScalar(mesh, runTime, "gammaIntEffIn", dimless, fgammaIntEff, zg);
    }
    autoPtr<volScalarField> epsP = mkScalar(mesh, runTime, "epsilon", dimensionSet(0, 2, -3, 0, 0), feps, zg);
    autoPtr<volScalarField> omP = mkScalar(mesh, runTime, "omega", dimensionSet(0, 0, -1, 0, 0), fomega, zg);
    autoPtr<volScalarField> ntP = mkScalar(mesh, runTime, "nuTilda", dimensionSet(0, 2, -1, 0, 0), fnuTilda, zg);
    mkScalar(mesh, runTime, "nut", dimensionSet(0, 2, -1, 0, 0), fzero, word(calculatedFvPatchScalarField::typeName));
    mkScalar(mesh, runTime, "flm", dimensionSet(0, 4, -4, 0, 0), fflm, zg);
    mkScalar(mesh, runTime, "fmm", dimensionSet(0, 4, -4, 0, 0), ffmm, zg);
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
    if (isLM)
    {
        dumpS("ReThetat", reThetatP().primitiveField());
        dumpS("gammaInt", gammaIntP().primitiveField());
        dumpS("gammaIntEff_in", gEffInP().primitiveField());
    }

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

    // nut as validate() left it (the model's nut_ is overwritten by correct())
    const volScalarField nutPre("nutPre", nut);
    if (model == "dynamicKEqn")
    {
        // the REAL Ck and Ce of the model object on the same frozen state, called before
        // correct() changes nut; KK is formed with the same filter class as the model's
        const RealDynKAccess& rm =
            static_cast<const RealDynKAccess&>(dynamic_cast<const RealDynK&>(turbulence()));
        autoPtr<LESfilter> F0p;
        if (filterName == "simple") F0p.reset(new simpleFilter(mesh));
        else if (filterName == "laplace") F0p.reset(new laplaceFilter(mesh, 2.0));
        else F0p.reset(new anisotropicFilter(mesh, 2.0));
        const LESfilter& F0 = F0p();
        const volSymmTensorField Dsymm0(symm(gradU));
        const volSymmTensorField Ddev0(devSymm(gradU));
        const volScalarField KKnut0(0.5*(F0(magSqr(U)) - magSqr(F0(U))));
        volScalarField KK0(0.5*(F0(magSqr(U)) - magSqr(F0(U))));
        KK0.clamp_min(SMALL);
        dumpVF("R_Ck_symm", rm.Ck(Dsymm0, KKnut0));
        dumpVF("R_Ck_dev", rm.Ck(Ddev0, KK0));
        dumpVF("R_Ce", rm.Ce(Ddev0, KK0));
    }

    // ---- the REAL correct(): matrices and local fields are printed by the
    //      dumpSolver instead of being solved ------------------------------
    if (isLM)
    {
        // the model's own gammaIntEff (zero from its constructor) is set to the frozen input so
        // the first correct() exercises the Pk / epsilonByk overrides (README slice 8)
        volScalarField::Internal& gE = mesh.lookupObjectRef<volScalarField::Internal>("gammaIntEff");
        gE.field() = gEffInP().primitiveField();
    }
    if (model != "Smagorinsky" && model != "WALE" && model != "LESfilters")
    {
        turbulence->correct();
    }
    if (isLM)
    {
        dumpS("gammaIntEff_real", mesh.lookupObject<volScalarField::Internal>("gammaIntEff").field());
        dumpS("ReThetat_after", mesh.lookupObject<volScalarField>("ReThetat").primitiveField());
        dumpS("gammaInt_after", mesh.lookupObject<volScalarField>("gammaInt").primitiveField());
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
    else if (model == "kOmegaSSTLM")
    {
        // RASModels/kOmegaSSTLM/kOmegaSSTLM.C on Base/kOmegaSST/kOmegaSSTBase.C (v2506 defaults)
        const dimensionedScalar alphaK1("aK1", dimless, 0.85), alphaK2("aK2", dimless, 1.0),
            alphaOmega1("aO1", dimless, 0.5), alphaOmega2("aO2", dimless, 0.856),
            gamma1("g1", dimless, 5.0/9.0), gamma2("g2", dimless, 0.44),
            beta1("b1", dimless, 0.075), beta2("b2", dimless, 0.0828),
            betaStar("bs", dimless, 0.09), a1("a1", dimless, 0.31),
            b1("bb1", dimless, 1.0), c1("c1", dimless, 10.0);
        const dimensionedScalar ca1_("ca1", dimless, 2), ca2_("ca2", dimless, 0.06),
            ce1_("ce1", dimless, 1), ce2_("ce2", dimless, 50), cThetat_("cThetat", dimless, 0.03),
            sigmaThetat_("sigmaThetat", dimless, 2);
        const scalar lambdaErr_ = 1e-6;
        const dimensionedScalar deltaU_("deltaU", dimVelocity, SMALL);
        const volScalarField& y = wallDist::New(mesh).y();
        const volScalarField& ReThetat_ = reThetatP();
        const volScalarField& gammaInt_ = gammaIntP();
        const volScalarField::Internal& gammaIntEff_ = gEffInP().internalField();

        const volScalarField S2(2*magSqr(symm(gradU)));
        volScalarField::Internal GbyNu0(gradU && devTwoSymm(gradU));
        const volScalarField::Internal G(nut*GbyNu0);

        const volScalarField CDkOmega
        (
            (2*alphaOmega2)*(fvc::grad(k) & fvc::grad(omega))/omega
        );
        // F1 = max(SST F1, F3)
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
        const volScalarField F1sst(tanh(pow4(arg1)));
        const volScalarField Ry(y*sqrt(k)/nu);
        const volScalarField F3(exp(-pow(Ry/120.0, 8)));
        const volScalarField F1(max(F1sst, F3));
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

        const volScalarField::Internal gamma(F1()*(gamma1 - gamma2) + gamma2);
        const volScalarField::Internal beta(F1()*(beta1 - beta2) + beta2);
        const volScalarField::Internal GbyNu
        (
            min(GbyNu0, (c1/a1)*betaStar*omega()*max(a1*omega(), b1*F23()*sqrt(S2())))
        );
        const volScalarField::Internal PkSst(min(G, (c1*betaStar)*k()*omega()));
        const volScalarField::Internal Pk(gammaIntEff_*PkSst);
        const volScalarField::Internal epsByk
        (
            clamp(gammaIntEff_, scalarMinMax(0.1, 1))*(betaStar*omega())
        );
        const volScalarField::Internal Eom(gamma*GbyNu);
        const volScalarField::Internal SuSpOm1(((2.0/3.0)*gamma)*divU);
        const volScalarField::Internal SpOm(beta*omega());
        const volScalarField::Internal SuSpOm2((F1() - scalar(1))*CDkOmega()/omega());
        const volScalarField::Internal SuSpK((2.0/3.0)*divU);
        dumpS("T_S2", S2.primitiveField());
        dumpS("T_CDkOmega", CDkOmega.primitiveField());
        dumpS("T_F1sst", F1sst.primitiveField());
        dumpS("T_F3", F3.primitiveField());
        dumpS("T_F1", F1.primitiveField());
        dumpS("T_F23", F23.primitiveField());
        dumpS("T_GbyNu0", GbyNu0.field());
        dumpS("T_G", G.field());
        dumpS("T_gamma", gamma.field());
        dumpS("T_beta", beta.field());
        dumpS("T_GbyNu", GbyNu.field());
        dumpS("T_PkSst", PkSst.field());
        dumpS("T_Pk", Pk.field());
        dumpS("T_omega_E", Eom.field());
        dumpS("T_omega_SuSp1", SuSpOm1.field());
        dumpS("T_omega_Sp", SpOm.field());
        dumpS("T_omega_SuSp2", SuSpOm2.field());
        dumpS("T_k_E", Pk.field());
        dumpS("T_k_SuSp", SuSpK.field());
        dumpS("T_k_Sp", epsByk.field());

        // --- the LM-specific terms, kOmegaSSTLM::correctReThetatGammaInt() and helpers ---
        const volScalarField::Internal& yI = y();
        const volScalarField::Internal omegaI(omega());
        const volScalarField::Internal& nuI = nu();
        const volScalarField::Internal Omega(sqrt(2*magSqr(skew(gradU()))));
        const volScalarField::Internal S(sqrt(2*magSqr(symm(gradU()))));
        const volScalarField::Internal Us(max(mag(U()), deltaU_));
        const volScalarField::Internal dUsds((U() & (U() & gradU()))/sqr(Us));
        dumpS("T_Omega", Omega.field());
        dumpS("T_S", S.field());
        dumpS("T_Us", Us.field());
        dumpS("T_dUsds", dUsds.field());

        // Fthetat
        dimensionedScalar deltaMin("deltaMin", dimLength, SMALL);
        volScalarField::Internal delta
        (
            max(375*Omega*nuI*ReThetat_()*yI/sqr(Us), deltaMin)
        );
        const volScalarField::Internal ReOmega(sqr(yI)*omegaI/nuI);
        const volScalarField::Internal Fwake(exp(-sqr(ReOmega/1e5)));
        const volScalarField::Internal Fthetat
        (
            min
            (
                max
                (
                    Fwake*exp(-pow4((yI/delta))),
                    (1 - sqr((gammaInt_() - 1.0/ce2_)/(1 - 1.0/ce2_)))
                ),
                scalar(1)
            )
        );
        dumpS("T_delta", delta.field());
        dumpS("T_Fwake", Fwake.field());
        dumpS("T_Fthetat", Fthetat.field());

        // ReThetac, Flength, ReThetat0 (explicit loops, as upstream)
        scalarField ReThetac(mesh.nCells()), Flength(mesh.nCells()), ReThetat0(mesh.nCells());
        scalarField lamIters(mesh.nCells());
        forAll(ReThetac, celli)
        {
            const scalar ReThetat = ReThetat_[celli];
            ReThetac[celli] =
                ReThetat <= 1870
              ?
                ReThetat
              - 396.035e-2
              + 120.656e-4*ReThetat
              - 868.230e-6*sqr(ReThetat)
              + 696.506e-9*pow3(ReThetat)
              - 174.105e-12*pow4(ReThetat)
              :
                ReThetat - 593.11 - 0.482*(ReThetat - 1870);
        }
        forAll(ReThetat_, celli)
        {
            const scalar ReThetat = ReThetat_[celli];
            if (ReThetat < 400)
            {
                Flength[celli] =
                    398.189e-1
                  - 119.270e-4*ReThetat
                  - 132.567e-6*sqr(ReThetat);
            }
            else if (ReThetat < 596)
            {
                Flength[celli] =
                    263.404
                  - 123.939e-2*ReThetat
                  + 194.548e-5*sqr(ReThetat)
                  - 101.695e-8*pow3(ReThetat);
            }
            else if (ReThetat < 1200)
            {
                Flength[celli] = 0.5 - 3e-4*(ReThetat - 596);
            }
            else
            {
                Flength[celli] = 0.3188;
            }
            const scalar Fsublayer =
                ::exp(-sqr(sqr(yI[celli])*omegaI[celli]/(200*nuI[celli])));
            Flength[celli] = Flength[celli]*(1 - Fsublayer) + 40*Fsublayer;
        }
        forAll(ReThetat0, celli)
        {
            const scalar Tu
            (
                max(100*::sqrt((2.0/3.0)*k[celli])/Us[celli], scalar(0.027))
            );
            scalar lambda = 0;
            scalar lambdaErr;
            scalar thetat;
            label iter = 0;
            do
            {
                const scalar lambda0 = lambda;
                if (Tu <= 1.3)
                {
                    const scalar Flambda =
                        dUsds[celli] <= 0
                      ?
                        1
                      - (
                         - 12.986*lambda
                         - 123.66*sqr(lambda)
                         - 405.689*pow3(lambda)
                        )*::exp(-::pow(Tu/1.5, 1.5))
                      :
                        1
                      + 0.275*(1 - ::exp(-35*lambda))
                       *::exp(-Tu/0.5);
                    thetat =
                        (1173.51 - 589.428*Tu + 0.2196/sqr(Tu))
                       *Flambda*nuI[celli]
                       /Us[celli];
                }
                else
                {
                    const scalar Flambda =
                        dUsds[celli] <= 0
                      ?
                        1
                      - (
                          -12.986*lambda
                          -123.66*sqr(lambda)
                          -405.689*pow3(lambda)
                        )*::exp(-::pow(Tu/1.5, 1.5))
                      :
                        1
                      + 0.275*(1 - ::exp(-35*lambda))
                       *::exp(-2*Tu);
                    thetat =
                        331.50*::pow((Tu - 0.5658), -0.671)
                       *Flambda*nuI[celli]/Us[celli];
                }
                lambda = sqr(thetat)/nuI[celli]*dUsds[celli];
                lambda = clamp(lambda, scalar(-0.1), scalar(0.1));
                lambdaErr = mag(lambda - lambda0);
                ++iter;
            } while (lambdaErr > lambdaErr_);
            ReThetat0[celli] = max(thetat*Us[celli]/nuI[celli], scalar(20));
            lamIters[celli] = scalar(iter);
        }
        dumpS("T_ReThetac", ReThetac);
        dumpS("T_Flength", Flength);
        dumpS("T_ReThetat0", ReThetat0);
        dumpS("T_lambdaIters", lamIters);

        // ReThetat equation
        const volScalarField::Internal tt(500*nuI/sqr(Us));
        const volScalarField::Internal Pthetat((cThetat_/tt)*(1 - Fthetat));
        dumpS("T_Pthetat", Pthetat.field());
        const volScalarField::Internal ReThetatE(Pthetat*mkInt(mesh, runTime, ReThetat0));
        dumpS("T_ReThetat_E", ReThetatE.field());
        // gammaInt equation
        const volScalarField::Internal Rev(sqr(yI)*S/nuI);
        const volScalarField::Internal RT(k()/(nuI*omegaI));
        const volScalarField::Internal ReThetacI(mkInt(mesh, runTime, ReThetac));
        const volScalarField::Internal FlengthI(mkInt(mesh, runTime, Flength));
        const volScalarField::Internal Fonset1(Rev/(2.193*ReThetacI));
        const volScalarField::Internal Fonset2(min(max(Fonset1, pow4(Fonset1)), scalar(2)));
        const volScalarField::Internal Fonset3(max(1 - pow3(RT/2.5), scalar(0)));
        const volScalarField::Internal Fonset(max(Fonset2 - Fonset3, scalar(0)));
        const volScalarField::Internal Pgamma(ca1_*FlengthI*S*sqrt(gammaInt_()*Fonset));
        const volScalarField::Internal Fturb(exp(-pow4(0.25*RT)));
        const volScalarField::Internal Egamma(ca2_*Omega*Fturb*gammaInt_());
        const volScalarField::Internal Freattach(exp(-pow4(RT/20.0)));
        const volScalarField::Internal gammaSep
        (
            min(2*max(Rev/(3.235*ReThetacI) - 1, scalar(0))*Freattach, scalar(2))
           *Fthetat
        );
        const volScalarField::Internal gammaIntEffNew(max(gammaInt_(), gammaSep));
        dumpS("T_Rev", Rev.field());
        dumpS("T_RT", RT.field());
        dumpS("T_Fonset", Fonset.field());
        dumpS("T_Pgamma", Pgamma.field());
        dumpS("T_Fturb", Fturb.field());
        dumpS("T_Egamma", Egamma.field());
        dumpS("T_Freattach", Freattach.field());
        dumpS("T_gammaSep", gammaSep.field());
        dumpS("T_gammaIntEff", gammaIntEffNew.field());

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
        const volScalarField DRe("DReThetatEff", sigmaThetat_*(nut + nu));
        const volScalarField Dg("DgammaIntEff", nut + nu);
        volScalarField reT("ReThetat", ReThetat_);
        volScalarField gaI("gammaInt", gammaInt_);
        fvScalarMatrix reEqn
        (
            fvm::ddt(reT) + fvm::div(phi, reT) - fvm::laplacian(DRe, reT)
         ==
            ReThetatE - fvm::Sp(Pthetat, reT)
        );
        auditMatrix("ReThetat", reEqn);
        fvScalarMatrix gaEqn
        (
            fvm::ddt(gaI) + fvm::div(phi, gaI) - fvm::laplacian(Dg, gaI)
         ==
            Pgamma - fvm::Sp(ce1_*Pgamma, gaI)
          + Egamma - fvm::Sp(ce2_*Egamma, gaI)
        );
        auditMatrix("gammaInt", gaEqn);
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
    else if (model == "dynamicKEqn")
    {
        // LESModels/dynamicKEqn/dynamicKEqn.C: Ck, Ce, correctNut(), correct(); delta cubeRootVol.
        const scalar widthCoeff = 2.0;
        const simpleFilter sF(mesh);
        autoPtr<LESfilter> Fp;
        if (filterName == "simple") Fp.reset(new simpleFilter(mesh));
        else if (filterName == "laplace") Fp.reset(new laplaceFilter(mesh, widthCoeff));
        else Fp.reset(new anisotropicFilter(mesh, widthCoeff));
        const LESfilter& F = Fp();
        const volScalarField& delta = mesh.lookupObject<volScalarField>("delta");
        dumpS("T_delta", delta.primitiveField());
        dumpVF("nutpre", nutPre);
        dumpVF("nut_after", nut);
        dumpVF("nu_f", nu);
        dumpVF("delta_f", delta);
        // correctNut(): D = symm(grad U), KK unclamped
        const volSymmTensorField Dsymm(symm(gradU));
        const volScalarField KKnut(0.5*(F(magSqr(U)) - magSqr(F(U))));
        autoPtr<volSymmTensorField> LLs, MMs;
        const volScalarField CkSymm(dynCk(sF, F, U, delta, Dsymm, KKnut, LLs, MMs));
        const volScalarField nutValidate(CkSymm*sqrt(k)*delta);
        // correct(): D = devSymm(grad U), KK clamped
        const volSymmTensorField Ddev(devSymm(gradU));
        volScalarField KK(0.5*(F(magSqr(U)) - magSqr(F(U))));
        KK.clamp_min(SMALL);
        autoPtr<volSymmTensorField> LLd, MMd;
        const volScalarField CkDev(dynCk(sF, F, U, delta, Ddev, KK, LLd, MMd));
        const volScalarField CeF(dynCe(sF, F, nutPre, nu, delta, Ddev, KK));
        const volScalarField nutAfter(CkDev*sqrt(k)*delta);
        const volScalarField G("G", 2.0*nutPre*(gradU && Ddev));
        const volScalarField::Internal SuSpK((2.0/3.0)*divU);
        const volScalarField SpK(CeF*sqrt(k)/delta);
        dumpVF("T_D_symm", Dsymm);
        dumpVF("T_m_dev", volScalarField(-2.0*delta*sqrt(max(KK, dimensionedScalar(KK.dimensions(), Zero)))));
        dumpVF("T_FD_dev", volSymmTensorField(F(Ddev)));
        dumpVF("T_D_dev", Ddev);
        dumpVF("T_KK_nut", KKnut);
        dumpVF("T_KK", KK);
        dumpVF("T_LL", LLd());
        dumpVF("T_MM_symm", MMs());
        dumpVF("T_MM_dev", MMd());
        dumpVF("T_Ck_symm", CkSymm);
        dumpVF("T_Ck_dev", CkDev);
        dumpVF("T_Ce", CeF);
        dumpS("T_nut_validate", nutValidate.primitiveField());
        dumpS("T_nut_after", nutAfter.primitiveField());
        dumpS("T_G", G.primitiveField());
        dumpS("T_k_SuSp", SuSpK.field());
        dumpS("T_k_Sp", SpK.primitiveField());
        const volScalarField Dk("DkEff", nutPre + nu);
        fvScalarMatrix kEqn
        (
            fvm::ddt(kP()) + fvm::div(phi, kP()) - fvm::laplacian(Dk, kP())
         ==
            G - fvm::SuSp(SuSpK, kP()) - fvm::Sp(SpK, kP())
        );
        auditMatrix("k", kEqn);
    }
    else if (model == "dynamicLagrangian")
    {
        // LESModels/dynamicLagrangian/dynamicLagrangian.C correct(), correctNut(gradU);
        // delta cubeRootVol, theta 1.5 (the default), alpha = rho = 1 (geometricOneField)
        const scalar widthCoeff = 2.0;
        autoPtr<LESfilter> Fp;
        if (filterName == "simple") Fp.reset(new simpleFilter(mesh));
        else if (filterName == "laplace") Fp.reset(new laplaceFilter(mesh, widthCoeff));
        else Fp.reset(new anisotropicFilter(mesh, widthCoeff));
        const LESfilter& F = Fp();
        const volScalarField& delta = mesh.lookupObject<volScalarField>("delta");
        const volScalarField& flm = mesh.lookupObject<volScalarField>("flm");
        const volScalarField& fmm = mesh.lookupObject<volScalarField>("fmm");
        const scalar theta = 1.5;
        volSymmTensorField S(devSymm(gradU));
        volScalarField magS(mag(S));
        volVectorField Uf(F(U));
        volSymmTensorField Sf(devSymm(fvc::grad(Uf)));
        volScalarField magSf(mag(Sf));
        volSymmTensorField L(dev(F(sqr(U)) - (sqr(F(U)))));
        volSymmTensorField M
        (
            2.0*sqr(delta)*(F(magS*S) - 4.0*magSf*Sf)
        );
        volScalarField invT
        (
            (1.0/(theta*delta))*pow(flm*fmm, 1.0/8.0)
        );
        volScalarField LM(L && M);
        fvScalarMatrix flmEqn
        (
            fvm::ddt(flm) + fvm::div(phi, flm)
         ==
            invT*LM - fvm::Sp(invT, flm)
        );
        auditMatrix("flm", flmEqn);
        volScalarField MMf(M && M);
        fvScalarMatrix fmmEqn
        (
            fvm::ddt(fmm) + fvm::div(phi, fmm)
         ==
            invT*MMf - fvm::Sp(invT, fmm)
        );
        auditMatrix("fmm", fmmEqn);
        const volScalarField nutT((flm/fmm)*sqr(delta)*mag(devSymm(gradU)));
        dumpVF("T_S", S);
        dumpVF("T_magS", magS);
        dumpVF("T_Uf", Uf);
        dumpVF("T_Sf", Sf);
        dumpVF("T_magSf", magSf);
        dumpVF("T_L", L);
        dumpVF("T_M", M);
        dumpVF("T_invT", invT);
        dumpVF("T_LM", LM);
        dumpVF("T_MM", MMf);
        dumpS("T_nut", nutT.primitiveField());
        dumpS("T_flm", flm.primitiveField());
        dumpS("T_fmm", fmm.primitiveField());
        dumpS("T_delta", delta.primitiveField());
        dumpVF("nut_after", nut);
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
