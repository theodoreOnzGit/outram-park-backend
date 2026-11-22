/*---------------------------------------------------------------------------*\
  gh:#1362 differential driver: prints, as IEEE-754 hex, the geometry, the
  finite-volume operators and the limiter functions of the *upstream*
  OpenFOAM-v2506 (ESI) libraries on a mesh made by upstream blockMesh /
  polyDualMesh, for comparison with outram-foam-basic-lib.

  This file is OUTRAM PARK's own driver; it only CALLS upstream
  (GPL-3.0-or-later, https://www.openfoam.com). No upstream source is copied
  into the repository.

  Output: one line per value,  "<key> <index> <hex64> [<hex64> <hex64>]"
  where each hex64 is the 16-digit big-endian bit pattern of an f64.

  The analytic inputs are POLYNOMIALS (no transcendental functions), so the
  inputs are exactly reproducible; the inputs are dumped too and the Rust side
  reads them back, so geometry and operator differences are separable.
\*---------------------------------------------------------------------------*/

#include "fvCFD.H"
#include "surfaceInterpolationScheme.H"
#include "fvcSurfaceIntegrate.H"
#include "NVDTVD.H"
#include "vanLeer.H"
#include "vanAlbada.H"
#include "Minmod.H"
#include "SuperBee.H"
#include "MUSCL.H"
#include "UMIST.H"
#include "OSPRE.H"
#include "limitedLinear.H"
#include "mixedFvPatchFields.H"
#include "fixedGradientFvPatchFields.H"
#include "fixedValueFvPatchFields.H"
#include <cstdio>
#include <cstring>
#include <cstdint>

using namespace Foam;

static void H(const char* key, label i, scalar a)
{
    uint64_t u;
    std::memcpy(&u, &a, 8);
    std::printf("%s %d %016llx\n", key, int(i), (unsigned long long)u);
}

static void H3(const char* key, label i, const vector& v)
{
    uint64_t u[3];
    scalar c[3] = {v.x(), v.y(), v.z()};
    std::memcpy(u, c, 24);
    std::printf
    (
        "%s %d %016llx %016llx %016llx\n", key, int(i),
        (unsigned long long)u[0], (unsigned long long)u[1],
        (unsigned long long)u[2]
    );
}

static scalar fT(const point& p)
{
    const scalar x = p.x(), y = p.y(), z = p.z();
    return 1.0 + 2.0*x - 3.0*y + 0.5*z + x*y + 0.25*y*z*z + x*x*z;
}

static vector fU(const point& p)
{
    const scalar x = p.x(), y = p.y(), z = p.z();
    return vector(1.0 + 0.5*y - 0.25*z*z, 0.3*x + 0.2*z, -0.1*x*y + 0.4);
}

static tensor fS(const point& p)
{
    const scalar x = p.x(), y = p.y(), z = p.z();
    return tensor
    (
        1.0 + 0.5*x - 0.25*y*z, 0.3*y + 0.1*x*x, -0.2*z + 0.15*x*y,
        0.4*x - 0.1*z*z, 2.0 - 0.3*y + 0.05*x*z, 0.25*x*y - 0.2*z,
        0.1*z + 0.2*y*y, -0.5*x + 0.3*y*z, 1.5 + 0.1*x - 0.2*y
    );
}

static void H9(const char* key, label i, const tensor& t)
{
    uint64_t u[9];
    scalar c[9] = {t.xx(), t.xy(), t.xz(), t.yx(), t.yy(), t.yz(), t.zx(), t.zy(), t.zz()};
    std::memcpy(u, c, 72);
    std::printf("%s %d", key, int(i));
    for (int k = 0; k < 9; ++k) std::printf(" %016llx", (unsigned long long)u[k]);
    std::printf("\n");
}

static scalar fG(const point& p)
{
    return 1.0 + 0.5*p.x() + 0.25*p.y()*p.y();
}

// Dump a per-face quantity: internal faces then boundary faces, in face order
static void dumpFace(const char* key, const surfaceScalarField& s)
{
    const fvMesh& mesh = s.mesh();
    label k = 0;
    forAll(s.primitiveField(), fi)
    {
        H(key, k++, s.primitiveField()[fi]);
    }
    forAll(s.boundaryField(), pi)
    {
        forAll(s.boundaryField()[pi], fi)
        {
            H(key, k++, s.boundaryField()[pi][fi]);
        }
    }
    (void)mesh;
}

// Dump a per-face vector quantity: internal faces then boundary faces
static void dumpFaceV(const char* key, const surfaceVectorField& s)
{
    label k = 0;
    forAll(s.primitiveField(), fi) H3(key, k++, s.primitiveField()[fi]);
    forAll(s.boundaryField(), pi)
    {
        forAll(s.boundaryField()[pi], fi) H3(key, k++, s.boundaryField()[pi][fi]);
    }
}

// Dump a scalar-matrix: diag, upper, source, patch internal/boundary coeffs
static void dumpLap(const std::string& pre, fvScalarMatrix& L)
{
    forAll(L.diag(), c) H((pre + "_diag").c_str(), c, L.diag()[c]);
    forAll(L.upper(), f) H((pre + "_upper").c_str(), f, L.upper()[f]);
    forAll(L.source(), c) H((pre + "_source").c_str(), c, L.source()[c]);
    label k = 0;
    forAll(L.internalCoeffs(), pi)
    {
        forAll(L.internalCoeffs()[pi], fi)
            H((pre + "_intCoeff").c_str(), k++, L.internalCoeffs()[pi][fi]);
    }
    k = 0;
    forAll(L.boundaryCoeffs(), pi)
    {
        forAll(L.boundaryCoeffs()[pi], fi)
            H((pre + "_bndCoeff").c_str(), k++, L.boundaryCoeffs()[pi][fi]);
    }
    if (L.faceFluxCorrectionPtr())
    {
        dumpFace((pre + "_ffc").c_str(), *L.faceFluxCorrectionPtr());
    }
}

template<class Lim>
static void dumpLimiter(const char* name, const Lim& lim, const scalar* rs, label nr)
{
    const vector d(1, 0, 0);
    const NVDTVD nvd;
    std::string rname = std::string(name) + "_r";
    for (label i = 0; i < nr; ++i)
    {
        const vector gradcP((rs[i] + 1.0)/2.0, 0, 0);
        H(rname.c_str(), i, nvd.r(1, 0.0, 1.0, gradcP, gradcP, d));
        H(name, i, lim.limiter(0.5, 1.0, 0.0, 1.0, gradcP, gradcP, d));
    }
}

int main(int argc, char* argv[])
{
    #include "setRootCase.H"
    #include "createTime.H"
    #include "createMesh.H"

    const vectorField& Cf = mesh.Cf().primitiveField();
    const vectorField& C = mesh.C().primitiveField();
    const label nInt = mesh.nInternalFaces();

    // ---- geometry --------------------------------------------------------
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
    }
    {
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
    }
    {
        const vectorField& fc = mesh.faceCentres();
        const vectorField& fa = mesh.faceAreas();
        forAll(fc, i)
        {
            H3("Cf", i, fc[i]);
            H3("Sf", i, fa[i]);
            H("magSf", i, mag(fa[i]));
        }
        const vectorField& cc = mesh.cellCentres();
        const scalarField& cv = mesh.cellVolumes();
        forAll(cc, i)
        {
            H3("C", i, cc[i]);
            H("V", i, cv[i]);
        }
    }
    const surfaceScalarField& w = mesh.weights();
    dumpFace("weights", w);
    dumpFace("deltaCoeffs", mesh.deltaCoeffs());
    dumpFace("nonOrthDeltaCoeffs", mesh.nonOrthDeltaCoeffs());

    // ---- inputs -----------------------------------------------------------
    wordList bt(mesh.boundary().size(), fixedValueFvPatchScalarField::typeName);
    volScalarField T
    (
        IOobject("T", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
        mesh, dimensionedScalar(dimless, 0), bt
    );
    forAll(C, c) T.primitiveFieldRef()[c] = fT(C[c]);
    forAll(T.boundaryField(), pi)
    {
        scalarField vals(mesh.boundary()[pi].size());
        forAll(vals, fi) vals[fi] = fT(mesh.boundary()[pi].Cf()[fi]);
        T.boundaryFieldRef()[pi] == vals;
    }
    forAll(T.primitiveField(), c) H("T", c, T.primitiveField()[c]);

    surfaceScalarField phi
    (
        IOobject("phi", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
        mesh, dimensionedScalar(dimless, 0)
    );
    surfaceScalarField gam
    (
        IOobject("gam", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
        mesh, dimensionedScalar(dimless, 0)
    );
    forAll(Cf, f)
    {
        if (f < nInt)
        {
            phi.primitiveFieldRef()[f] = fU(Cf[f]) & mesh.Sf().primitiveField()[f];
            gam.primitiveFieldRef()[f] = fG(Cf[f]);
        }
    }
    forAll(phi.boundaryField(), pi)
    {
        const fvPatch& p = mesh.boundary()[pi];
        forAll(p, fi)
        {
            phi.boundaryFieldRef()[pi][fi] = fU(p.Cf()[fi]) & p.Sf()[fi];
            gam.boundaryFieldRef()[pi][fi] = fG(p.Cf()[fi]);
        }
    }
    dumpFace("phi", phi);
    dumpFace("gam", gam);

    // ---- operators --------------------------------------------------------
    // 1. Gauss linear gradient
    {
        tmp<volVectorField> g = fvc::grad(T);
        forAll(g().primitiveField(), c) H3("gradT", c, g().primitiveField()[c]);
    }
    // 2. interpolation schemes, positive and negative face flux
    {
        const char* names[] =
        {
            "linear", "upwind", "vanLeer", "Minmod",
            "linearUpwind grad(T)", "limitedLinear 1", "limitedLinear 0.5"
        };
        const char* keys[] =
        {
            "linear", "upwind", "vanLeer", "Minmod",
            "linearUpwind", "limitedLinear1", "limitedLinear05"
        };
        for (int s = 0; s < 7; ++s)
        {
            for (int sgn = 1; sgn >= -1; sgn -= 2)
            {
                surfaceScalarField ff
                (
                    IOobject("ff", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
                    mesh, dimensionedScalar(dimless, scalar(sgn))
                );
                IStringStream is(names[s]);
                tmp<surfaceInterpolationScheme<scalar>> sch =
                    surfaceInterpolationScheme<scalar>::New(mesh, ff, is);
                tmp<surfaceScalarField> r = sch().interpolate(T);
                std::string key = std::string("interp_") + keys[s] + (sgn > 0 ? "_pos" : "_neg");
                dumpFace(key.c_str(), r());
            }
        }
    }
    // 3. snGrad: orthogonal (deltaCoeffs) and uncorrected (nonOrthDeltaCoeffs)
    {
        // construct the scheme explicitly: fvc::snGrad(vf, name) would look the
        // name up in fvSchemes (and fall to its default), not parse it
        IStringStream isO("orthogonal");
        tmp<fv::snGradScheme<scalar>> so = fv::snGradScheme<scalar>::New(mesh, isO);
        tmp<surfaceScalarField> a = so().snGrad(T);
        dumpFace("snGrad_orthogonal", a());
        IStringStream isU("uncorrected");
        tmp<fv::snGradScheme<scalar>> su = fv::snGradScheme<scalar>::New(mesh, isU);
        tmp<surfaceScalarField> b = su().snGrad(T);
        dumpFace("snGrad_uncorrected", b());
    }
    // 4. divergence
    {
        tmp<volScalarField> d = fvc::div(phi);
        forAll(d().primitiveField(), c) H("divPhi", c, d().primitiveField()[c]);
        tmp<volScalarField> dt = fvc::div(phi, T, "div(phi,T)");
        forAll(dt().primitiveField(), c) H("divPhiT", c, dt().primitiveField()[c]);
    }
    // 5. Gauss linear uncorrected laplacian matrix
    {
        fvScalarMatrix L(fvm::laplacian(gam, T, "laplacian(gam,T)"));
        forAll(L.diag(), c) H("lap_diag", c, L.diag()[c]);
        forAll(L.upper(), f) H("lap_upper", f, L.upper()[f]);
        forAll(L.source(), c) H("lap_source", c, L.source()[c]);
        label k = 0;
        forAll(L.internalCoeffs(), pi)
        {
            forAll(L.internalCoeffs()[pi], fi)
                H("lap_intCoeff", k++, L.internalCoeffs()[pi][fi]);
        }
        k = 0;
        forAll(L.boundaryCoeffs(), pi)
        {
            forAll(L.boundaryCoeffs()[pi], fi)
                H("lap_bndCoeff", k++, L.boundaryCoeffs()[pi][fi]);
        }
    }


    // ---- gh:#1362 task 2 additions ---------------------------------------
    // 6. leastSquares gradient (fvSchemes: grad(Tls) leastSquares)
    {
        tmp<volVectorField> g = fvc::grad(T, "grad(Tls)");
        forAll(g().primitiveField(), c) H3("gradT_ls", c, g().primitiveField()[c]);
    }
    // 7. corrected / limited snGrad, and the non-orthogonal correction vectors
    {
        dumpFaceV("nonOrthCorrVec", mesh.nonOrthCorrectionVectors());
        const char* txt[] = {"corrected", "limited corrected 0.33", "limited corrected 0.5"};
        const char* key[] = {"corrected", "limited033", "limited05"};
        for (int i = 0; i < 3; ++i)
        {
            IStringStream is(txt[i]);
            tmp<fv::snGradScheme<scalar>> s = fv::snGradScheme<scalar>::New(mesh, is);
            dumpFace((std::string("snGrad_") + key[i]).c_str(), s().snGrad(T)());
            dumpFace((std::string("snGradCorr_") + key[i]).c_str(), s().correction(T)());
        }
    }
    // 8. corrected / limited laplacian matrices (fvSchemes: laplacian(gam,Tc), (gam,Tl))
    {
        mesh.setFluxRequired(T.name());
        {
            fvScalarMatrix L(fvm::laplacian(gam, T, "laplacian(gam,Tc)"));
            dumpLap("lapc", L);
        }
        {
            fvScalarMatrix L(fvm::laplacian(gam, T, "laplacian(gam,Tl)"));
            dumpLap("lapl", L);
        }
    }
    // 9. vector and tensor divergence (Gauss linear)
    {
        wordList bv(mesh.boundary().size(), fixedValueFvPatchVectorField::typeName);
        volVectorField U
        (
            IOobject("U", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
            mesh, dimensionedVector(dimless, Zero), bv
        );
        forAll(C, c) U.primitiveFieldRef()[c] = fU(C[c]);
        forAll(U.boundaryField(), pi)
        {
            vectorField vals(mesh.boundary()[pi].size());
            forAll(vals, fi) vals[fi] = fU(mesh.boundary()[pi].Cf()[fi]);
            U.boundaryFieldRef()[pi] == vals;
        }
        tmp<volVectorField> dU = fvc::div(phi, U, "div(phi,U)");
        forAll(dU().primitiveField(), c) H3("divPhiU", c, dU().primitiveField()[c]);

        wordList bs(mesh.boundary().size(), fixedValueFvPatchTensorField::typeName);
        volTensorField S
        (
            IOobject("S", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
            mesh, dimensionedTensor(dimless, Zero), bs
        );
        forAll(C, c) S.primitiveFieldRef()[c] = fS(C[c]);
        forAll(S.boundaryField(), pi)
        {
            tensorField vals(mesh.boundary()[pi].size());
            forAll(vals, fi) vals[fi] = fS(mesh.boundary()[pi].Cf()[fi]);
            S.boundaryFieldRef()[pi] == vals;
        }
        forAll(S.primitiveField(), c) H9("S", c, S.primitiveField()[c]);
        tmp<volVectorField> dS = fvc::div(S);
        forAll(dS().primitiveField(), c) H3("divS", c, dS().primitiveField()[c]);
    }
    // 10. mixed and fixedGradient patches: inlet fixedGradient (uniform),
    // outlet mixed (per face values), walls mixed (uniform values)
    {
        wordList bm(3);
        bm[0] = fixedGradientFvPatchScalarField::typeName;
        bm[1] = mixedFvPatchScalarField::typeName;
        bm[2] = mixedFvPatchScalarField::typeName;
        volScalarField Tm
        (
            IOobject("Tm", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
            mesh, dimensionedScalar(dimless, 0), bm
        );
        forAll(C, c) Tm.primitiveFieldRef()[c] = fT(C[c]);
        {
            fixedGradientFvPatchScalarField& g =
                refCast<fixedGradientFvPatchScalarField>(Tm.boundaryFieldRef()[0]);
            g.gradient() = 0.7;
        }
        {
            mixedFvPatchScalarField& m =
                refCast<mixedFvPatchScalarField>(Tm.boundaryFieldRef()[1]);
            const fvPatch& p = m.patch();
            forAll(p, fi)
            {
                const point& x = p.Cf()[fi];
                m.refValue()[fi] = fT(x);
                m.refGrad()[fi] = 0.3 + 0.1*x.y();
                m.valueFraction()[fi] = 0.2 + 0.3*x.z();
            }
        }
        {
            mixedFvPatchScalarField& m =
                refCast<mixedFvPatchScalarField>(Tm.boundaryFieldRef()[2]);
            m.refValue() = 2.5;
            m.refGrad() = -0.8;
            m.valueFraction() = 0.35;
        }
        Tm.correctBoundaryConditions();
        {
            label k = 0;
            forAll(Tm.boundaryField(), pi)
            {
                forAll(Tm.boundaryField()[pi], fi) H("Tm_bnd", k++, Tm.boundaryField()[pi][fi]);
            }
        }
        {
            tmp<volVectorField> g = fvc::grad(Tm);
            forAll(g().primitiveField(), c) H3("gradTm", c, g().primitiveField()[c]);
        }
        {
            surfaceScalarField ff
            (
                IOobject("ff", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
                mesh, dimensionedScalar(dimless, scalar(1))
            );
            IStringStream is("linear");
            tmp<surfaceInterpolationScheme<scalar>> sch =
                surfaceInterpolationScheme<scalar>::New(mesh, ff, is);
            dumpFace("interpm_linear", sch().interpolate(Tm)());
        }
        {
            IStringStream isU("uncorrected");
            tmp<fv::snGradScheme<scalar>> su = fv::snGradScheme<scalar>::New(mesh, isU);
            dumpFace("snGradm_uncorrected", su().snGrad(Tm)());
        }
        {
            fvScalarMatrix L(fvm::laplacian(gam, Tm, "laplacian(gam,Tm)"));
            dumpLap("lapm", L);
        }
    }

    // ---- limiter functions on a grid of r --------------------------------
    {
        scalar rs[400];
        label nr = 0;
        for (int i = 0; i < 300; ++i) rs[nr++] = -3.0 + i*0.0371;
        const scalar extra[] =
            {0, 1, 2, 0.5, -1, 1e-300, 1000, 0.25, 0.75, 3, 5, 1e-12, 1.5, 4};
        for (scalar e : extra) rs[nr++] = e;

        IStringStream isNone("");
        IStringStream isLL("1");
        IStringStream isLL5("0.5");
        vanLeerLimiter<NVDTVD> vl(isNone);
        vanAlbadaLimiter<NVDTVD> va(isNone);
        MinmodLimiter<NVDTVD> mm(isNone);
        SuperBeeLimiter<NVDTVD> sb(isNone);
        MUSCLLimiter<NVDTVD> mu(isNone);
        UMISTLimiter<NVDTVD> um(isNone);
        OSPRELimiter<NVDTVD> os(isNone);
        IStringStream isLLs("1e-16");
        limitedLinearLimiter<NVDTVD> llS(isLLs);
        limitedLinearLimiter<NVDTVD> ll1(isLL);
        limitedLinearLimiter<NVDTVD> ll5(isLL5);
        dumpLimiter("lim_vanLeer", vl, rs, nr);
        dumpLimiter("lim_vanAlbada", va, rs, nr);
        dumpLimiter("lim_Minmod", mm, rs, nr);
        dumpLimiter("lim_SuperBee", sb, rs, nr);
        dumpLimiter("lim_MUSCL", mu, rs, nr);
        dumpLimiter("lim_UMIST", um, rs, nr);
        dumpLimiter("lim_OSPRE", os, rs, nr);
        dumpLimiter("lim_limitedLinear1", ll1, rs, nr);
        dumpLimiter("lim_limitedLinear05", ll5, rs, nr);
        // k = 1e-16 is below upstream's `small` (1e-15): twoByk = 2/small. A grid of
        // tiny r separates `small` = 1e-15 from any other floor.
        const scalar tiny[] = {0, 1.1102230246251565e-16, 2.220446049250313e-16,
                               4.440892098500626e-16, 8.881784197001252e-16, 1e-15, 1e-14};
        dumpLimiter("lim_limitedLinearTiny", llS, tiny, 7);
    }
    return 0;
}
