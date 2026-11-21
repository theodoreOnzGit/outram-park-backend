//! `HEN_HOUSE/src/egs_parallel.mortran` (EGSnrc).
//!
//! Upstream: <https://github.com/nrc-cnrc/EGSnrc>, commit `bd49a47aae3938f49f4100b8e018091861408b19`
//! (read 2026-11-20, a shallow clone, never committed here). Licence: AGPL-3.0
//! (the file headers read version 3 or, at the licensee's option, later;
//! the SLAC-derived parts carry their own notice in the header).
//!
//! Support for parallel runs of the Mortran codes (422 lines), read in full
//! 2026-11-21.
//!
//! **Not ported, deliberately.** The file is lock-file job control for runs
//! that share an `EGS_HOME`: `egs_create_lockfile`, `egs_open_lockfile`,
//! `egs_remove_lockfile`, `egs_pjob_control` and `egs_pjob_finish` do all their
//! work through the C utilities `egs_create_control_file`,
//! `egs_open_control_file`, `egs_read_control_file`, `egs_write_control_file`,
//! `egs_rewind_control_file`, `egs_unlock_control_file`, `egs_remove_file` and
//! `egs_date_and_time`, and through a text control string kept in that file
//! (and the whole file is under `#ifdef HAVE_C_COMPILER`). The arithmetic in
//! it (how many histories a job runs next, the combination of `sum` and
//! `sum2` into `res` and `dres`) is inseparable from reading and writing that
//! shared file, and there is no physics. A `no_std` library has no file
//! system and no second process to share one with; a port would be a design
//! of its own (a job scheduler), not a translation.
//!
//! **Status: placeholder, nothing ported (gh:#976).** When code lands here it
//! carries the attribution header block (upstream project, file, commit,
//! copyright, licence) and a doc-comment `file:line` for every routine.
