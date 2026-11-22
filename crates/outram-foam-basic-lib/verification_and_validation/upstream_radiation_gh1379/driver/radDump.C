/*---------------------------------------------------------------------------*\
  gh:#1379 differential driver: runs the REAL OpenFOAM-v2506 (ESI) P1 radiation
  model (radiationModel::New(T) -> P1::correct()) on a case made by upstream
  blockMesh and prints, as IEEE-754 hex, the geometry, the inputs and the
  results (G, qr, Ru, Rp, the Marshak boundary coefficients, the ST and Sh
  energy-source matrices) for comparison with outram-foam-basic-lib::radiation.

  This file is OUTRAM PARK's own driver; it only CALLS upstream
  (GPL-3.0-or-later, https://www.openfoam.com). No upstream source is copied.

  Output: one line per value,  "<key> <index> <hex64> [<hex64> <hex64>]".
  Inputs are POLYNOMIALS (no transcendental functions): exactly reproducible.
  system/radDumpDict selects the T field: "polynomial" or "slab" (uniform T0
  with per-patch wall temperatures).
  -noGeom skips the geometry lines (same mesh as an earlier run).
\*---------------------------------------------------------------------------*/

#include "fvCFD.H"
#include "radiationModel.H"
#include "fvmSup.H"
#include "constants.H"
#include "mixedFvPatchFields.H"
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

// Temperature [K], Cpv [J/kg/K], he [J/kg]: polynomials of position
static scalar fT(const point& p)
{
    return 500.0 + 200.0*p.x() + 150.0*p.y() + 100.0*p.z() + 50.0*p.x()*p.y();
}
static scalar fCpv(const point& p)
{
    return 1000.0 + 20.0*p.x() + 10.0*p.y() + 5.0*p.z();
}
static scalar fHe(const point& p)
{
    return 300000.0 + 1000.0*p.x() - 500.0*p.y() + 250.0*p.z();
}

// Boundary-face dump: patch order, face order within the patch
static void dumpB(const char* key, const volScalarField::Boundary& bf)
{
    label k = 0;
    forAll(bf, pi) forAll(bf[pi], fi) H(key, k++, bf[pi][fi]);
}

static void dumpMatrix(const std::string& pre, const fvScalarMatrix& M)
{
    forAll(M.diag(), c) H((pre + "_diag").c_str(), c, M.diag()[c]);
    forAll(M.source(), c) H((pre + "_source").c_str(), c, M.source()[c]);
}

int main(int argc, char* argv[])
{
    argList::addBoolOption("noGeom", "skip the geometry dump");
    #include "setRootCase.H"
    #include "createTime.H"
    #include "createMesh.H"

    const bool noGeom = args.found("noGeom");
    const vectorField& C = mesh.C().primitiveField();
    const label nInt = mesh.nInternalFaces();

    std::printf("# nCells %d nFaces %d nInternalFaces %d\n",
        int(mesh.nCells()), int(mesh.nFaces()), int(nInt));
    H("sigma", 0, constant::physicoChemical::sigma.value());

    if (!noGeom)
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

    // ---- T --------------------------------------------------------------
    IOdictionary dumpDict
    (
        IOobject
        (
            "radDumpDict", runTime.system(), mesh,
            IOobject::MUST_READ, IOobject::NO_WRITE
        )
    );
    const word Tfield(dumpDict.get<word>("Tfield"));

    wordList bt(mesh.boundary().size(), fixedValueFvPatchScalarField::typeName);
    volScalarField T
    (
        IOobject("T", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
        mesh, dimensionedScalar(dimTemperature, 0), bt
    );
    if (Tfield == "polynomial")
    {
        forAll(C, c) T.primitiveFieldRef()[c] = fT(C[c]);
        forAll(T.boundaryField(), pi)
        {
            scalarField vals(mesh.boundary()[pi].size());
            forAll(vals, fi) vals[fi] = fT(mesh.boundary()[pi].Cf()[fi]);
            T.boundaryFieldRef()[pi] == vals;
        }
    }
    else
    {
        const scalar T0 = dumpDict.get<scalar>("T0");
        const dictionary& Twall = dumpDict.subDict("Twall");
        T.primitiveFieldRef() = T0;
        forAll(T.boundaryField(), pi)
        {
            T.boundaryFieldRef()[pi] ==
                scalarField
                (
                    mesh.boundary()[pi].size(),
                    Twall.get<scalar>(mesh.boundary()[pi].name())
                );
        }
    }
    forAll(T.primitiveField(), c) H("T", c, T.primitiveField()[c]);
    dumpB("Tb", T.boundaryField());

    // ---- the real P1 model ---------------------------------------------
    autoPtr<radiation::radiationModel> rad(radiation::radiationModel::New(T));
    rad->correct();

    const volScalarField& G = mesh.lookupObject<volScalarField>("G");
    const volScalarField& qr = mesh.lookupObject<volScalarField>("qr");
    forAll(G.primitiveField(), c) H("G", c, G.primitiveField()[c]);
    dumpB("Gb", G.boundaryField());
    dumpB("qrb", qr.boundaryField());
    {
        label k = 0;
        forAll(G.boundaryField(), pi)
        {
            const mixedFvPatchScalarField* m =
                dynamic_cast<const mixedFvPatchScalarField*>(&G.boundaryField()[pi]);
            forAll(G.boundaryField()[pi], fi)
            {
                H("vf", k, m ? m->valueFraction()[fi] : -1.0);
                H("refv", k, m ? m->refValue()[fi] : -1.0);
                H("refg", k, m ? m->refGrad()[fi] : -1.0);
                H("dcb", k, mesh.boundary()[pi].deltaCoeffs()[fi]);
                ++k;
            }
        }
    }
    {
        tmp<DimensionedField<scalar, volMesh>> tRu(rad->Ru());
        tmp<volScalarField> tRp(rad->Rp());
        forAll(tRu(), c) H("Ru", c, tRu()[c]);
        forAll(tRp().primitiveField(), c) H("Rp", c, tRp().primitiveField()[c]);
    }

    // ---- ST(T): the temperature-equation source ---------------------------
    {
        tmp<fvScalarMatrix> tST(rad->ST(T));
        dumpMatrix("st", tST());
    }

    // ---- Sh(thermo, he): radiationModel::Sh() with Cpv, he supplied directly
    // (the three lines of radiationModel::Sh, with basicThermo::Cpv() replaced by a
    // polynomial field, since building a basicThermo adds nothing to the radiation
    // algebra under test).
    {
        wordList cb(mesh.boundary().size(), fixedValueFvPatchScalarField::typeName);
        volScalarField Cpv
        (
            IOobject("Cpv", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
            mesh, dimensionedScalar(dimensionSet(0, 2, -2, -1, 0), 0), cb
        );
        volScalarField he
        (
            IOobject("he", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
            mesh, dimensionedScalar(dimensionSet(0, 2, -2, 0, 0), 0), cb
        );
        forAll(C, c)
        {
            Cpv.primitiveFieldRef()[c] = fCpv(C[c]);
            he.primitiveFieldRef()[c] = fHe(C[c]);
        }
        forAll(Cpv.boundaryField(), pi)
        {
            scalarField cv(mesh.boundary()[pi].size()), hv(mesh.boundary()[pi].size());
            forAll(cv, fi)
            {
                cv[fi] = fCpv(mesh.boundary()[pi].Cf()[fi]);
                hv[fi] = fHe(mesh.boundary()[pi].Cf()[fi]);
            }
            Cpv.boundaryFieldRef()[pi] == cv;
            he.boundaryFieldRef()[pi] == hv;
        }
        forAll(Cpv.primitiveField(), c) H("Cpv", c, Cpv.primitiveField()[c]);
        forAll(he.primitiveField(), c) H("he", c, he.primitiveField()[c]);

        const volScalarField T3(pow3(T));
        tmp<fvScalarMatrix> tSh
        (
            rad->Ru()
          - fvm::Sp(4.0*rad->Rp()*T3/Cpv, he)
          - rad->Rp()*T3*(T - 4.0*he/Cpv)
        );
        dumpMatrix("sh", tSh());
    }

    return 0;
}
