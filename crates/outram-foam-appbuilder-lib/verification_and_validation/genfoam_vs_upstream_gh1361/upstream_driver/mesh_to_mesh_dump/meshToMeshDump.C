/*---------------------------------------------------------------------------*\
  gh:#651 (epic #1216, round 28) driver. NOT part of GeN-Foam: a small program
  written for the OUTRAM PARK differential comparison.

  What it measures: the `meshToMesh` objects GeN-Foam 652b3da0cc55's `meshHandler`
  builds (meshHandler.C:192-201):

      meshToMesh(meshes_[i], meshes_[j], imCellVolumeWeight, pmAABB, false)

  for the five (i, j) of the shipped 3D_SmallESFR multiRegionCouplingDict
  (fluid,neutro) (neutro,fluid) (neutro,thermoMechanical) (thermoMechanical,fluid)
  (thermoMechanical,neutro), on OpenFOAM-ESI v2506's own meshToMesh.

  Per object it dumps (17 significant digits, `%.17g`):
    "L <obj> <list> <cell> n a w a w ..."  srcToTgt / tgtToSrc addressing + (normalised) weights
    "V <obj> <overlapVolume>"              meshToMesh::V()
    "M <obj> <name> v1 v2 ..."             mapped cell fields (see below)

  Mapped fields. GeN-Foam moves field f (on region B = tgt) onto g (on region A = src)
  with mappingList_[A][B].mapTgtToSrc(f, plusEqOp, g), meshHandlerTemplates.C:85,133. Here
  the same call is made on the cell lists (the internal-field overload,
  meshToMeshTemplates.C:261), and mapSrcToTgt (:40) for the other direction, with
  closed-form (+ - * / only, no libm) fields:

      f(x,y,z) = 300 + 100*x - 40*y + 25*z + 10*x*y*z          scalar, on the SOURCE-side cell centres
      g(x,y,z) = 5 + 3*x - 2*y + z/7                           scalar, the pre-existing target value
      u(x,y,z) = (x*y + 1, z - x/3, y*z*x - 2)                 vector source field
      w(x,y,z) = (0.5 + x, y/5, 2 - z)                         vector pre-existing target value

  The pre-existing value matters: the operator is `result *= (1 - sum w); result += w * value`.

  Also: the same constructor with normalise = false (raw overlap volumes, "srcToTgtRaw"/"tgtToSrcRaw" and "V <obj>__raw"), and
  two objects between neutroRegion and a copy of it moved by a closed-form point displacement (genuine partial overlap):

      p' = p + (0.03 (1 + 0.5 y z - 0.2 x), 0.025 (1 - 0.4 x z + 0.3 y), 0.02 (1 + 0.3 x y - 0.1 z))

  Usage:  meshToMeshDump -case <case> <out.txt>
  Licence: GPL-3.0-or-later, as the OUTRAM PARK workspace; written fresh.
\*---------------------------------------------------------------------------*/

#include "argList.H"
#include "Time.H"
#include "fvMesh.H"
#include "meshToMesh.H"
#include <cstdio>
#include <ctime>

using namespace Foam;

static double fS(const point& c) { const double x=c.x(), y=c.y(), z=c.z(); return 300 + 100*x - 40*y + 25*z + 10*x*y*z; }
static double gS(const point& c) { const double x=c.x(), y=c.y(), z=c.z(); return 5 + 3*x - 2*y + z/7; }
static vector uV(const point& c) { const double x=c.x(), y=c.y(), z=c.z(); return vector(x*y + 1, z - x/3, y*z*x - 2); }
static vector wV(const point& c) { const double x=c.x(), y=c.y(), z=c.z(); return vector(0.5 + x, y/5, 2 - z); }

static void dumpList(FILE* fp, const char* obj, const char* name, const labelListList& a, const scalarListList& w)
{
    forAll(a, i)
    {
        fprintf(fp, "L %s %s %d %d", obj, name, int(i), int(a[i].size()));
        forAll(a[i], j) fprintf(fp, " %d %.17g", int(a[i][j]), w[i][j]);
        fprintf(fp, "\n");
    }
}

static void dumpScalar(FILE* fp, const char* obj, const char* name, const scalarField& v)
{
    fprintf(fp, "M %s %s", obj, name);
    forAll(v, i) fprintf(fp, " %.17g", v[i]);
    fprintf(fp, "\n");
}

static void dumpVector(FILE* fp, const char* obj, const char* name, const vectorField& v)
{
    fprintf(fp, "M %s %s", obj, name);
    forAll(v, i) fprintf(fp, " %.17g %.17g %.17g", v[i].x(), v[i].y(), v[i].z());
    fprintf(fp, "\n");
}

int main(int argc, char* argv[])
{
    argList::noParallel();
    argList::addArgument("out.txt");
    #include "setRootCase.H"
    #include "createTime.H"
    const fileName outName(args[1]);
    FILE* fp = fopen(outName.c_str(), "w");

    const wordList names({"fluidRegion", "neutroRegion", "thermoMechanicalRegion", "neutroMovedRegion"});
    PtrList<fvMesh> meshes(names.size());
    forAll(names, i)
    {
        meshes.set(i, new fvMesh(IOobject(names[i], runTime.timeName(), runTime, IOobject::MUST_READ)));
    }
    // neutroMovedRegion is a second copy of neutroRegion's mesh, moved by a closed-form point displacement
    // (+ - * / only) so that it genuinely PARTIALLY overlaps the original: real fvMesh::movePoints geometry.
    {
        fvMesh& mv = meshes[3];
        pointField p(mv.points());
        pointField q(p.size());
        forAll(p, i)
        {
            const double x = p[i].x(), y = p[i].y(), z = p[i].z();
            q[i] = point
            (
                x + 0.03*(1 + 0.5*y*z - 0.2*x),
                y + 0.025*(1 - 0.4*x*z + 0.3*y),
                z + 0.02*(1 + 0.3*x*y - 0.1*z)
            );
        }
        mv.movePoints(q);
    }
    // (src region, tgt region) of mappingList_[src][tgt]: the five of the shipped dictionary, then both ways of
    // original <-> moved neutroRegion (genuine partial overlap)
    const label pairs[7][2] = {{0,1},{1,0},{1,2},{2,0},{2,1},{1,3},{3,1}};
    for (int k = 0; k < 7; ++k)
    {
        const fvMesh& A = meshes[pairs[k][0]];
        const fvMesh& B = meshes[pairs[k][1]];
        const word obj(word(A.name()) + "__" + word(B.name()));
        const std::time_t t0 = std::time(nullptr);
        meshToMesh m(A, B, meshToMesh::interpolationMethod::imCellVolumeWeight, meshToMesh::procMapMethod::pmAABB, false);
        Info<< obj << " built in " << long(std::time(nullptr) - t0) << " s, V = " << m.V() << endl;
        fprintf(fp, "V %s %.17g\n", obj.c_str(), m.V());
        dumpList(fp, obj.c_str(), "srcToTgtAddr", m.srcToTgtCellAddr(), m.srcToTgtCellWght());
        dumpList(fp, obj.c_str(), "tgtToSrcAddr", m.tgtToSrcCellAddr(), m.tgtToSrcCellWght());
        {
            // the same method with normalise = false (the 7-argument constructor): the RAW overlap volumes
            meshToMesh r
            (
                A, B, meshToMesh::interpolationMethod::imCellVolumeWeight,
                HashTable<word>(), wordList(), meshToMesh::procMapMethod::pmAABB, false
            );
            fprintf(fp, "V %s__raw %.17g\n", obj.c_str(), r.V());
            dumpList(fp, obj.c_str(), "srcToTgtRaw", r.srcToTgtCellAddr(), r.srcToTgtCellWght());
            dumpList(fp, obj.c_str(), "tgtToSrcRaw", r.tgtToSrcCellAddr(), r.tgtToSrcCellWght());
        }

        // mapTgtToSrc: field of B onto A (what GeN-Foam does), result pre-set to g(C_A)
        {
            scalarField fB(B.nCells()); forAll(fB, i) fB[i] = fS(B.cellCentres()[i]);
            scalarField rA(A.nCells()); forAll(rA, i) rA[i] = gS(A.cellCentres()[i]);
            m.mapTgtToSrc(fB, plusEqOp<scalar>(), rA);
            dumpScalar(fp, obj.c_str(), "tgtToSrcScalar", rA);
            vectorField uB(B.nCells()); forAll(uB, i) uB[i] = uV(B.cellCentres()[i]);
            vectorField rAv(A.nCells()); forAll(rAv, i) rAv[i] = wV(A.cellCentres()[i]);
            m.mapTgtToSrc(uB, plusEqOp<vector>(), rAv);
            dumpVector(fp, obj.c_str(), "tgtToSrcVector", rAv);
        }
        // mapSrcToTgt: field of A onto B
        {
            scalarField fA(A.nCells()); forAll(fA, i) fA[i] = fS(A.cellCentres()[i]);
            scalarField rB(B.nCells()); forAll(rB, i) rB[i] = gS(B.cellCentres()[i]);
            m.mapSrcToTgt(fA, plusEqOp<scalar>(), rB);
            dumpScalar(fp, obj.c_str(), "srcToTgtScalar", rB);
        }
    }
    fclose(fp);
    return 0;
}
