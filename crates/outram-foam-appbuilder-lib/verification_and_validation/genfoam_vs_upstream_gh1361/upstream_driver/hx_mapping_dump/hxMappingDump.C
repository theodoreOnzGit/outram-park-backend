/*---------------------------------------------------------------------------*\
  gh:#1903 (round 52) driver. NOT part of GeN-Foam: written for the OUTRAM PARK differential comparison.

  Builds, with OpenFOAM-ESI v2506's own classes, what GeN-Foam 652b3da's
  heatExchanger::constructSecondaryHX (heatExchanger.C:310-431) builds for a heat exchanger whose
  primary and secondary zones are cell zones of the SAME region: the volume-weighted centres of the
  two zones (the same loops over V() and C()), delta = pCOV - sCOV, the translated copy of the whole
  mesh (points + delta, the same faces and cells) and

      meshToMesh(translatedMesh, mesh, imCellVolumeWeight, pmAABB, false)

  and dumps, per exchanger, "D <name> dx dy dz pV sV" and for every PRIMARY cell
  "L <name> <cell> n a w a w ..." (tgtToSrcCellAddr, tgtToSrcCellWght: what mapSrcToTgt uses), %.17g.

  It is NOT the instrumented binary; its delta is compared with the real binary's log line.
  Usage: hxMappingDump -case <case> <out.txt> <name> <primary> <secondary> [<name> <primary> <secondary> ...]
  Licence: AGPL-3.0-only (workspace), written fresh.
\*---------------------------------------------------------------------------*/

#include "argList.H"
#include "Time.H"
#include "fvMesh.H"
#include "meshToMesh.H"
#include <cstdio>
#include <string>
#include <vector>

using namespace Foam;

int main(int argc, char* argv[])
{
    argList::noParallel();
    argList::addArgument("out.txt");
    argList::addArgument("name:primary:secondary[,name:primary:secondary...]");
    #include "setRootCase.H"
    #include "createTime.H"
    const fileName outName(args[1]);
    FILE* fp = fopen(outName.c_str(), "w");
    fvMesh mesh(IOobject("fluidRegion", runTime.timeName(), runTime, IOobject::MUST_READ));
    const std::string spec(args[2]);
    std::vector<std::string> items;
    {
        size_t b = 0;
        while (b <= spec.size())
        {
            size_t e = spec.find(',', b);
            if (e == std::string::npos) e = spec.size();
            items.push_back(spec.substr(b, e - b));
            b = e + 1;
        }
    }
    for (const std::string& it : items)
    {
        const size_t c1 = it.find(':');
        const size_t c2 = it.find(':', c1 + 1);
        const word name(it.substr(0, c1));
        const labelList primaryCells(mesh.cellZones()[word(it.substr(c1 + 1, c2 - c1 - 1))]);
        const labelList secondaryCells(mesh.cellZones()[word(it.substr(c2 + 1))]);
        vector pCOV(vector::zero);
        vector sCOV(vector::zero);
        const vectorField& primC(mesh.C());
        const scalarField& primV(mesh.V());
        scalar pV(0);
        scalar sV(0);
        forAll(primaryCells, i)
        {
            const label& celli(primaryCells[i]);
            pCOV += primV[celli]*primC[celli];
            pV += primV[celli];
        }
        pCOV /= pV;
        forAll(secondaryCells, i)
        {
            const label& celli(secondaryCells[i]);
            sCOV += primV[celli]*primC[celli];
            sV += primV[celli];
        }
        sCOV /= sV;
        vector delta(pCOV - sCOV);
        fprintf(fp, "D %s %.17g %.17g %.17g %.17g %.17g\n", name.c_str(), delta.x(), delta.y(), delta.z(), pV, sV);

        const pointField& pointsRef(mesh.points());
        const faceList& facesRef(mesh.faces());
        const cellList& cellsRef(mesh.cells());
        pointField points(pointsRef.size());
        faceList faces(facesRef.size());
        cellList cells(cellsRef.size());
        forAll(points, i) { points[i] = pointsRef[i] + delta; }
        forAll(faces, i) { faces[i] = facesRef[i]; }
        forAll(cells, i) { cells[i] = cellsRef[i]; }
        autoPtr<fvMesh> translatedMeshPtr
        (
            new Foam::fvMesh
            (
                IOobject("translatedMesh", runTime.constant(), runTime, IOobject::NO_READ, IOobject::NO_WRITE),
                std::move(points), std::move(faces), std::move(cells)
            )
        );
        meshToMesh m
        (
            translatedMeshPtr(), mesh, meshToMesh::interpolationMethod::imCellVolumeWeight, meshToMesh::procMapMethod::pmAABB, false
        );
        Info<< name << ": V = " << m.V() << endl;
        const labelListList& addr = m.tgtToSrcCellAddr();
        const scalarListList& wght = m.tgtToSrcCellWght();
        forAll(primaryCells, i)
        {
            const label c = primaryCells[i];
            fprintf(fp, "L %s %d %d", name.c_str(), int(c), int(addr[c].size()));
            forAll(addr[c], j) { fprintf(fp, " %d %.17g", int(addr[c][j]), wght[c][j]); }
            fprintf(fp, "\n");
        }
    }
    fclose(fp);
    return 0;
}
