//! `pMesh` (cfMesh, gh:#1476): reads `<case>/system/meshDict` and its
//! `surfaceFile`, runs the port's `pMesh` workflow and writes
//! `<case>/constant/polyMesh`. See [`outram_foam_cli::cfmesh`].

fn main() {
    outram_foam_cli::cfmesh::cfmesh_main(outram_park_fork_cfmesh::mesh_dict::Generator::PMesh);
}
