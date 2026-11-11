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
        const char* names[] = {"linear", "upwind", "vanLeer", "Minmod"};
        for (int s = 0; s < 4; ++s)
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
                std::string key = std::string("interp_") + names[s] + (sgn > 0 ? "_pos" : "_neg");
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
    }
    return 0;
}
