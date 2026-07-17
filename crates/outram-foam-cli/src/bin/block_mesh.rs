//! `blockMesh` — structured hex meshing from a blockMeshDict
//!
//! Scaffold: parses the OpenFOAM `-case` option; the case-interface wiring is
//! implemented by the CLI fleet (see the `op-` beads).

use clap::Parser;
use outram_foam_cli::{CaseArgs, CliError};

fn main() {
    let args = CaseArgs::parse();
    if let Err(e) = run(&args) {
        eprintln!("blockMesh: error: {e}");
        std::process::exit(1);
    }
}

fn run(args: &CaseArgs) -> Result<(), CliError> {
    let _case = args.case_dir()?;
    Err(CliError::NotWired("blockMesh"))
}
