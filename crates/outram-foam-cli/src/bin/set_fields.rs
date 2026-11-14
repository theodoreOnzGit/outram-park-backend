//! `setFields`: set field values on cell selections through
//! `system/setFieldsDict` (gh:#1384). A thin wrapper over
//! [`outram_foam_cli::set_fields::run_set_fields`]; `-flavour esi` writes ESI
//! v2506's bytes. CLI glue: exercised through the library function in
//! `tests/set_fields_vs_esi_gh1384.rs`, not through the process.

use outram_foam_cli::{CliError, WriteFlavour};

fn main() {
    let args = outram_foam_cli::flavour_args();
    if let Err(e) = run(&args) {
        eprintln!("setFields: error: {e}");
        std::process::exit(1);
    }
}

fn run(args: &outram_foam_cli::FlavourArgs) -> Result<(), CliError> {
    let case = args.case.case_dir()?;
    let flavour = WriteFlavour::parse(&args.flavour)?;
    let report = outram_foam_cli::set_fields::run_set_fields(&case, flavour)?;
    for l in &report.log {
        println!("{l}");
    }
    println!("\nEnd");
    Ok(())
}
