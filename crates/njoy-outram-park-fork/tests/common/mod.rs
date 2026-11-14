//! Helpers shared by the deck tests that run NJOY2016 beside this crate.

use std::path::Path;
use std::process::Command;

/// Today's date as NJOY2016's `dater` writes it (`util.f90:72-94`): `mm/dd/yy`
/// of the local clock, which is what gfortran's `date_and_time` reads.
fn local_mmddyy() -> String {
    let out = Command::new("date").arg("+%m/%d/%y").output().expect("run `date`");
    String::from_utf8(out.stdout).expect("utf-8 date").trim().to_string()
}

/// Run NJOY2016 (`bin < deck > log` in `dir`) on a day that does not change
/// during the run, and return `(success, date)`: the date (`mm/dd/yy`) every
/// `dater` call of that run returned (gh:#1549).
///
/// ACER and DTFR stamp the clock's date into their output. A comparison that
/// gives this crate the same date ([`njoy_outram_park_fork::deck::DeckOptions::run_date`])
/// cannot fail on a run that crosses midnight. The date is read before and
/// after NJOY runs; if they differ, NJOY's units may carry either, so it is run
/// once more (it then starts after midnight, and a run does not last a day).
pub fn run_njoy_dated(bin: &Path, dir: &Path) -> (bool, String) {
    for _ in 0..2 {
        let before = local_mmddyy();
        let st = Command::new("sh")
            .arg("-c")
            .arg(format!("{} < deck > log 2>&1", bin.display()))
            .current_dir(dir)
            .status()
            .unwrap();
        let after = local_mmddyy();
        if before == after || !st.success() {
            return (st.success(), after);
        }
    }
    panic!("NJOY2016 ran across midnight twice in a row in {}", dir.display());
}
