/*---------------------------------------------------------------------------*\
  gh:#1529 differential driver: runs the REAL OpenFOAM-v2506 (ESI) viewFactor
  radiation model (radiationModel::New(T) -> viewFactor::calculate(), direct
  dense solve) on a case prepared by upstream blockMesh, faceAgglomerate and
  viewFactorsGen, and prints as IEEE-754 hex: the face areas, the emissivities,
  the F matrix and globalFaceFaces as read by upstream, finalAgglom, and qr on
  every boundary face after TWO calculate() calls (the second after T has been
  scaled, which exercises the cached-LU path of constantEmissivity).

  This file is OUTRAM PARK's own driver; it only CALLS upstream
  (GPL-3.0-or-later, https://www.openfoam.com). No upstream source is copied.

  Output: one line per value,  "<key> <index> <hex64>".
  system/vfDumpDict: Twall { patch value; ... }, Tscale2 (T of the 2nd call).
\*---------------------------------------------------------------------------*/

#include "fvCFD.H"
#include "radiationModel.H"
#include "constants.H"
#include "fixedValueFvPatchFields.H"
#include "scalarListIOList.H"
#include "labelListIOList.H"
#include "boundaryRadiationProperties.H"
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

static void dumpB(const char* key, const volScalarField::Boundary& bf)
{
    label k = 0;
    forAll(bf, pi) forAll(bf[pi], fi) H(key, k++, bf[pi][fi]);
}

int main(int argc, char* argv[])
{
    #include "setRootCase.H"
    #include "createTime.H"
    #include "createMesh.H"

    std::printf("# nCells %d nFaces %d\n", int(mesh.nCells()), int(mesh.nFaces()));
    H("sigma", 0, constant::physicoChemical::sigma.value());

    forAll(mesh.boundary(), pi)
    {
        std::printf("patch %d %s %d %d\n", int(pi),
            mesh.boundary()[pi].name().c_str(),
            int(mesh.boundary()[pi].start()), int(mesh.boundary()[pi].size()));
    }
    {
        label k = 0;
        forAll(mesh.boundary(), pi)
        {
            const scalarField& sf = mesh.magSf().boundaryField()[pi];
            forAll(sf, fi) H("magSf", k++, sf[fi]);
        }
    }

    IOdictionary dumpDict
    (
        IOobject
        (
            "vfDumpDict", runTime.system(), mesh,
            IOobject::MUST_READ, IOobject::NO_WRITE
        )
    );
    const dictionary& Twall = dumpDict.subDict("Twall");
    const scalar Tscale2 = dumpDict.get<scalar>("Tscale2");

    wordList bt(mesh.boundary().size(), fixedValueFvPatchScalarField::typeName);
    volScalarField T
    (
        IOobject("T", runTime.timeName(), mesh, IOobject::NO_READ, IOobject::NO_WRITE),
        mesh, dimensionedScalar(dimTemperature, 300), bt
    );
    forAll(T.boundaryField(), pi)
    {
        T.boundaryFieldRef()[pi] ==
            scalarField
            (
                mesh.boundary()[pi].size(),
                Twall.get<scalar>(mesh.boundary()[pi].name())
            );
    }
    dumpB("Tb", T.boundaryField());

    // inputs as upstream reads them
    {
        const scalarListIOList F
        (
            IOobject("F", mesh.facesInstance(), mesh, IOobject::MUST_READ, IOobject::NO_WRITE)
        );
        const labelListIOList gff
        (
            IOobject("globalFaceFaces", mesh.facesInstance(), mesh, IOobject::MUST_READ, IOobject::NO_WRITE)
        );
        forAll(F, i)
        {
            std::printf("Frow %d %d\n", int(i), int(F[i].size()));
            forAll(F[i], j) H("F", j, F[i][j]);
            forAll(gff[i], j) std::printf("gff %d %d\n", int(j), int(gff[i][j]));
        }
    }

    autoPtr<radiation::radiationModel> rad(radiation::radiationModel::New(T));

    {
        const radiation::boundaryRadiationProperties& brp =
            radiation::boundaryRadiationProperties::New(mesh);
        label k = 0;
        forAll(mesh.boundary(), pi)
        {
            const scalarField& Tp = T.boundaryField()[pi];
            const tmp<scalarField> e = brp.emissivity(pi, 0, nullptr, &Tp);
            forAll(e(), fi) H("emis", k++, e()[fi]);
        }
    }

    rad->calculate();
    const volScalarField& qr = mesh.lookupObject<volScalarField>("qr");
    dumpB("qr1", qr.boundaryField());

    forAll(T.boundaryField(), pi)
    {
        T.boundaryFieldRef()[pi] == Tscale2*T.boundaryField()[pi];
    }
    dumpB("Tb2", T.boundaryField());
    rad->calculate();
    dumpB("qr2", qr.boundaryField());

    return 0;
}
