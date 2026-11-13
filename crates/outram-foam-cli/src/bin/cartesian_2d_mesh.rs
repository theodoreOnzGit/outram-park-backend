//! `cartesian2DMesh` (cfMesh, gh:#1476): reads `<case>/system/meshDict` and its
//! `surfaceFile`, runs the port's `cartesian2DMesh` workflow and writes
//! `<case>/constant/polyMesh`. See [`outram_foam_cli::cfmesh`].

fn main() {
    outram_foam_cli::cfmesh::cfmesh_main(outram_park_fork_cfmesh::mesh_dict::Generator::Cartesian2DMesh);
}
