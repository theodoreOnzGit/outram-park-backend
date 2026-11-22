// SPDX-License-Identifier: AGPL-3.0-only
//
// Attribution (RESEARCH_INTEGRITY_AND_PROVENANCE.md): port of EGSnrc.
//   Upstream project: EGSnrc, https://github.com/nrc-cnrc/EGSnrc
//   Upstream file:    HEN_HOUSE/src/egs_parallel.mortran
//   Upstream commit:  bd49a47aae3938f49f4100b8e018091861408b19
//   Copyright:        (C) 2015 National Research Council Canada (Iwan
//                     Kawrakow, Blake Walters, Frederic Tessier, Ernesto
//                     Mainegra-Hing)
//   Upstream licence: AGPL-3.0-or-later; this port is AGPL-3.0-only.
//! `HEN_HOUSE/src/egs_parallel.mortran` (EGSnrc, 422 lines): the job control
//! of parallel runs, as pure state transitions.
//!
//! **Status: first port, compared with a compiled EGSnrc (gh:#1697)**: the
//! control strings and the values every job obtains agree exactly with real
//! processes sharing one lock file (`docs/egsnrc_phase2_differential.md`,
//! "egs_parallel"). ~~Not ported, deliberately: no `no_std` seam.~~
//! **CORRECTED 2026-11-21:** the file is lock-file job control, but the
//! decisions in it are a state transition over the text of the lock file,
//! which separates cleanly from the file I/O.
//!
//! | upstream | here |
//! |---|---|
//! | `egs_pjob_control` (`:105-322`) | [`pjob_control`] |
//! | `egs_pjob_finish` (`:324-420`) | [`pjob_finish`] |
//! | `$control_format` `(2i15,i4,2(1pe16.8),1pe13.5,0pf7.2,2x,8i5)` (`:204`) | [`PjobControlRecord::to_control_string`] |
//! | `read(control_string,*) n_left,nn_tot,n_job,tmp,tmp2,res,dres,(t_dum(i),i=1,8)` | [`PjobControlRecord::parse`] |
//! | `egs_create_lockfile`, `egs_open_lockfile`, `egs_remove_lockfile` (`:45-103`) | **the caller** (see below) |
//!
//! **The seam.** The upstream routines call the C utilities of
//! `HEN_HOUSE/cutils/egs_c_utils.c` (`egs_create_control_file`,
//! `egs_open_control_file`, `egs_rewind_control_file` (open the lock),
//! `egs_read_control_file`, `egs_write_control_file`,
//! `egs_unlock_control_file`, `egs_close_control_file`, `egs_remove_file`) and
//! `egs_date_and_time`. Those stay with the caller: it reads the lock file
//! (while holding the lock) and passes its text as `file_text`; it writes
//! back the text in [`PjobAction`] (`n_write = lnblnk1(control_string) + 1`
//! bytes: the text and **one blank**, which [`PjobAction::file_bytes`] gives)
//! and unlocks. Nothing here touches a file, a clock or `std`. The
//! `$egs_fatal` branches (read, write, lock failures) are `EgsError::Fatal`;
//! the C-status failures are the caller's.
//!
//! **Types.** `$REAL` is `f64`, `$LONG_INT` `i64`, as in the
//! `-fdefault-real-8` reference build.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::egsnrc_macros::EgsError;
use super::pegs4_routines_mortran::ListReader;

/// `$N_CHUNKS` (`egsnrc.macros:3555`): each job takes `ncase / n_parallel / 10`
/// histories at a time.
pub const N_CHUNKS: i64 = 10;

/// The record the jobs keep in the lock file:
/// `n_left, n_tot, n_job, sum, sum2, res, dres, t_first(1..8)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PjobControlRecord {
    /// Histories not yet handed out.
    pub n_left: i64,
    /// Histories finished so far (all jobs).
    pub n_tot: i64,
    /// Jobs running.
    pub n_job: i32,
    /// Score summed over all jobs.
    pub sum: f64,
    /// Score squared, summed.
    pub sum2: f64,
    /// Combined result.
    pub res: f64,
    /// Its relative uncertainty, %.
    pub dres: f64,
    /// `t_first`: the start time of the first job (`egs_date_and_time`).
    pub times: [i32; 8],
}

/// Fortran `Iw`: right-justified, `*` fill on overflow.
fn fmt_i(v: i64, w: usize) -> String {
    let s = format!("{v}");
    if s.len() > w { "*".repeat(w) } else { format!("{s:>w$}") }
}

/// Fortran `1PEw.d`: `d.ddd...E+ee` (`+ddd` without the `E` for a three-digit
/// exponent), right-justified in `w`, `*` fill on overflow.
fn fmt_e(x: f64, w: usize, d: usize) -> String {
    if !x.is_finite() {
        return "*".repeat(w);
    }
    let s = format!("{x:.d$e}");
    let (mant, exp) = s.split_once('e').unwrap_or((&s, "0"));
    let e: i32 = exp.parse().unwrap_or(0);
    let sign = if e < 0 { '-' } else { '+' };
    let body = if e.abs() <= 99 { format!("{mant}E{sign}{:02}", e.abs()) } else { format!("{mant}{sign}{:03}", e.abs()) };
    if body.len() > w { "*".repeat(w) } else { format!("{body:>w$}") }
}

/// Fortran `0PFw.d`.
fn fmt_f(x: f64, w: usize, d: usize) -> String {
    if !x.is_finite() {
        return "*".repeat(w);
    }
    let s = format!("{x:.d$}");
    if s.len() > w { "*".repeat(w) } else { format!("{s:>w$}") }
}

/// A list-directed real (`D` exponents, and a sign standing for `E` as in
/// `1.0000000+100`, which `1PE` writes for a three-digit exponent).
fn parse_real(t: &str) -> Option<f64> {
    let mut u = t.replace(['D', 'd'], "e");
    if !u.contains(['e', 'E']) {
        if let Some(p) = u.char_indices().skip(1).find(|(_, c)| *c == '+' || *c == '-').map(|(i, _)| i) {
            u.insert(p, 'e');
        }
    }
    u.parse::<f64>().ok()
}

impl PjobControlRecord {
    /// `write(control_string, $control_format) n_left,n_tot,n_job,sum,sum2,res,dres,(t(i),i=1,8)`
    /// with trailing blanks removed (`lnblnk1`). Format
    /// `(2i15,i4,2(1pe16.8),1pe13.5,0pf7.2,2x,8i5)`.
    #[must_use]
    pub fn to_control_string(&self) -> String {
        let mut s = String::new();
        s.push_str(&fmt_i(self.n_left, 15));
        s.push_str(&fmt_i(self.n_tot, 15));
        s.push_str(&fmt_i(i64::from(self.n_job), 4));
        s.push_str(&fmt_e(self.sum, 16, 8));
        s.push_str(&fmt_e(self.sum2, 16, 8));
        s.push_str(&fmt_e(self.res, 13, 5));
        s.push_str(&fmt_f(self.dres, 7, 2));
        s.push_str("  ");
        for t in self.times {
            s.push_str(&fmt_i(i64::from(t), 5));
        }
        let n = s.trim_end().len();
        s.truncate(n);
        s
    }

    /// `read(control_string,*) n_left,n_tot,n_job,sum,sum2,res,dres,(t(i),i=1,8)`:
    /// 15 blank-separated items ([`ListReader`]); a NUL counts as a blank.
    ///
    /// # Errors
    ///
    /// [`EgsError::Fatal`] for a missing or malformed item (the
    /// `:error_lockfile_read:` and `:error2_lockfile_read:` branches).
    pub fn parse(text: &str) -> Result<Self, EgsError> {
        const BAD: EgsError = EgsError::Fatal("Failed to read from job control file");
        let clean = text.replace('\0', " ");
        let mut rd = ListReader::new(&clean);
        let t = rd.items(15).map_err(|_| BAD)?;
        let int = |k: usize| t[k].parse::<i64>().map_err(|_| BAD);
        let real = |k: usize| parse_real(&t[k]).ok_or(BAD);
        let mut times = [0_i32; 8];
        for (i, slot) in times.iter_mut().enumerate() {
            *slot = i32::try_from(int(7 + i)?).map_err(|_| BAD)?;
        }
        Ok(Self {
            n_left: int(0)?,
            n_tot: int(1)?,
            n_job: i32::try_from(int(2)?).map_err(|_| BAD)?,
            sum: real(3)?,
            sum2: real(4)?,
            res: real(5)?,
            dres: real(6)?,
            times,
        })
    }
}

/// What the caller must do with the lock file after a call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PjobAction {
    /// Nothing: no lock file is used (`is_uniform_run`, `n_parallel <= 0`).
    None,
    /// Job `first_parallel`, first call: create the lock file
    /// (`egs_create_lockfile`), write this control string, unlock.
    Create(String),
    /// Rewind (lock), write this control string, unlock.
    Update(String),
    /// `egs_pjob_finish`, last job: close and remove the lock file.
    Remove,
}

impl PjobAction {
    /// The bytes upstream writes for a control string: the text and the one
    /// blank at `lnblnk1 + 1` (`n_write` bytes of the blank-padded string).
    #[must_use]
    pub fn file_bytes(&self) -> Option<String> {
        match self {
            Self::Create(s) | Self::Update(s) => Some(format!("{s} ")),
            _ => None,
        }
    }
}

/// The run options `egs_init` read (`COMMON/EGS-IO/`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PjobConfig {
    /// `n_parallel` (`-P`).
    pub n_parallel: i32,
    /// `i_parallel` (`-j`).
    pub i_parallel: i32,
    /// `first_parallel` (`-f`, default 1).
    pub first_parallel: i32,
    /// `is_uniform_run` (`-u`): no lock file.
    pub is_uniform_run: bool,
}

/// The variables `egs_pjob_control` keeps between calls (`save first_time,
/// n_last` and the `COMMON` variable `n_max_parallel`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PjobJobState {
    /// `first_time` (`data first_time/.true./`).
    pub first_time: bool,
    /// `n_last`: histories of the chunk this job was last given.
    pub n_last: i64,
    /// `n_max_parallel`: the largest `n_job` this job has seen.
    pub n_max_parallel: i32,
}

impl Default for PjobJobState {
    fn default() -> Self {
        Self { first_time: true, n_last: 0, n_max_parallel: 0 }
    }
}

/// What one call of [`pjob_control`] gives back.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PjobControlResult {
    /// `n_run`: histories to run now; 0 means finish.
    pub n_run: i64,
    /// `n_left`.
    pub n_left: i64,
    /// `n_tot`, in and out.
    pub n_tot: i64,
    /// `res` and `dres`, or `None` where upstream does not assign them
    /// (`n_parallel <= 0`, `is_uniform_run`).
    pub res_dres: Option<(f64, f64)>,
}

/// `res = tmp/n_tot; dres = tmp2/n_tot - res*res; IF(res > 0 & dres > 0) dres =
/// sqrt(dres/(n_tot-1))/res*100 ELSE dres = 99.9`, with `n_tot <= 1` giving
/// `res = 0, dres = 99.9` (`:244-248`, `:278-283`).
fn statistics(sum: f64, sum2: f64, n_tot: i64) -> (f64, f64) {
    if n_tot > 1 {
        let nt = n_tot as f64;
        let res = sum / nt;
        let mut dres = sum2 / nt - res * res;
        if res > 0.0 && dres > 0.0 {
            dres = petir::real::Real::sqrt(dres / ((n_tot - 1) as f64)) / res * 100.0;
        } else {
            dres = 99.9;
        }
        (res, dres)
    } else {
        (0.0, 99.9)
    }
}

/// `egs_pjob_control` (`egs_parallel.mortran:105-322`): the next chunk of
/// histories for this job, and the new lock-file text.
///
/// `ncase` is the total number of histories, `n_tot` the histories done
/// before this call (on the first call of job `first_parallel`: those of a
/// previous run; on the first call of the other jobs: 0), `sum`/`sum2` the
/// score since the last call (on the first call of job `first_parallel`: the
/// score of the previous run). `file_text` is the lock file as read by the
/// caller; it is not needed on the first call of job `first_parallel`,
/// `is_uniform_run` and `n_parallel <= 0` (pass `None`).
///
/// # Errors
///
/// [`EgsError::Fatal`] when the lock-file text cannot be read, or none was
/// given where upstream reads one.
pub fn pjob_control(
    cfg: &PjobConfig,
    state: &mut PjobJobState,
    ncase: i64,
    n_tot_in: i64,
    sum: f64,
    sum2: f64,
    file_text: Option<&str>,
) -> Result<(PjobControlResult, PjobAction), EgsError> {
    // the option without a lock file
    if cfg.is_uniform_run {
        let mut n_run = 0;
        if state.first_time {
            n_run = ncase / i64::from(cfg.n_parallel);
            state.first_time = false;
        }
        return Ok((PjobControlResult { n_run, n_left: 0, n_tot: n_tot_in, res_dres: None }, PjobAction::None));
    }
    if cfg.n_parallel <= 0 {
        return Ok((PjobControlResult { n_run: ncase, n_left: 0, n_tot: n_tot_in, res_dres: None }, PjobAction::None));
    }
    if state.first_time {
        state.n_last = 0;
    }
    let mut n_run = ncase / i64::from(cfg.n_parallel) / N_CHUNKS;
    let mut n_left = ncase;
    if state.first_time {
        if n_run > n_left {
            n_run = n_left;
        }
        n_left = ncase - n_run;
        if cfg.i_parallel == cfg.first_parallel {
            // I'm the first job: create the job control file
            let (res, dres) = statistics(sum, sum2, n_tot_in);
            let rec = PjobControlRecord { n_left, n_tot: n_tot_in, n_job: 1, sum, sum2, res, dres, times: [0; 8] };
            state.first_time = false;
            state.n_max_parallel = 1;
            state.n_last = n_run;
            // `t_first` belongs to the caller (`egs_date_and_time`); see `PjobConfig`
            return Ok((
                PjobControlResult { n_run, n_left, n_tot: n_tot_in, res_dres: Some((res, dres)) },
                PjobAction::Create(rec.to_control_string()),
            ));
        }
    }
    // here for all calls except the first call from job `first_parallel`
    let text = file_text.ok_or(EgsError::Fatal("egs_pjob_control: the job control file was not read"))?;
    let mut rec = PjobControlRecord::parse(text)?;
    let mut nn_tot = rec.n_tot + state.n_last;
    let tmp = rec.sum + sum;
    let tmp2 = rec.sum2 + sum2;
    let mut n_job = rec.n_job;
    // first call: one more job running
    if state.first_time {
        state.first_time = false;
        n_job += 1;
        nn_tot += n_tot_in;
    }
    let n_tot = nn_tot;
    let (res, dres) = statistics(tmp, tmp2, n_tot);
    // take another chunk of histories
    n_left = rec.n_left;
    if n_run > n_left {
        n_run = n_left;
        n_left = 0;
    } else {
        n_left -= n_run;
    }
    state.n_last = n_run;
    rec = PjobControlRecord { n_left, n_tot, n_job, sum: tmp, sum2: tmp2, res, dres, times: rec.times };
    if n_job > state.n_max_parallel {
        state.n_max_parallel = n_job;
    }
    Ok((PjobControlResult { n_run, n_left, n_tot, res_dres: Some((res, dres)) }, PjobAction::Update(rec.to_control_string())))
}

/// What `egs_pjob_finish` decided.
#[derive(Debug, Clone, PartialEq)]
pub struct PjobFinishResult {
    /// `n_job` as upstream returns it: jobs still running after this one
    /// (0 for the last job).
    pub n_job: i32,
    /// This job is the last: it completes the calculation (`egs_open_units`,
    /// the summary). The caller prints it with `n_parallel`, the job's
    /// `n_max_parallel` and the elapsed time from `t_start`.
    pub last: bool,
    /// `t_start` read from the control record (the first job's start time);
    /// `None` for `is_uniform_run`.
    pub t_start: Option<[i32; 8]>,
}

/// `egs_pjob_finish` (`egs_parallel.mortran:324-420`): this job is done.
/// Decrements `n_job` in the control record and says whether it was the last.
///
/// `file_text` is the lock file as read (ignored for `is_uniform_run`).
///
/// Upstream then ends at `:complete_calculation:` with `i_parallel = 0; call
/// egs_open_units(.false.)` (`egs_parallel.mortran:394`). `cfg` is borrowed and
/// not changed here: a caller that names files by `i_parallel` after the last
/// finish, as upstream's later output does, sets it to 0 itself (gh:#1682).
///
/// # Errors
///
/// [`EgsError::Fatal`] when the lock-file text cannot be read.
pub fn pjob_finish(cfg: &PjobConfig, file_text: Option<&str>) -> Result<(PjobFinishResult, PjobAction), EgsError> {
    if cfg.is_uniform_run {
        // only the last job completes
        let last = cfg.i_parallel == cfg.n_parallel;
        return Ok((PjobFinishResult { n_job: i32::from(!last), last, t_start: None }, PjobAction::None));
    }
    let text = file_text.ok_or(EgsError::Fatal("egs_pjob_finish: the job control file was not read"))?;
    let mut rec = PjobControlRecord::parse(text)?;
    rec.n_job -= 1;
    if rec.n_job > 0 {
        // other jobs still running
        let s = rec.to_control_string();
        return Ok((PjobFinishResult { n_job: rec.n_job, last: false, t_start: None }, PjobAction::Update(s)));
    }
    // the last job removes the job control file
    Ok((PjobFinishResult { n_job: rec.n_job, last: true, t_start: Some(rec.times) }, PjobAction::Remove))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The control strings of the compiled upstream (the first records of the
    /// 3-job run in `docs/egsnrc_phase2_differential.md`).
    const FIRST: &str = "            967              0   1  0.00000000E+00  0.00000000E+00  0.00000E+00  99.90    101  102  103  104  105  106  107  108";
    const SECOND: &str = "            934              0   2  0.00000000E+00  0.00000000E+00  0.00000E+00  99.90    101  102  103  104  105  106  107  108";
    const THIRD: &str = "            901             33   2  3.99300000E+01  5.65653000E+01  1.21000E+00   7.30    101  102  103  104  105  106  107  108";

    fn times() -> [i32; 8] {
        [101, 102, 103, 104, 105, 106, 107, 108]
    }

    /// The format `(2i15,i4,2(1pe16.8),1pe13.5,0pf7.2,2x,8i5)` writes the
    /// strings the compiled upstream wrote, and the reader reads them back.
    #[test]
    fn control_format_and_reader_round_trip() {
        let rec = PjobControlRecord { n_left: 901, n_tot: 33, n_job: 2, sum: 39.93, sum2: 56.5653, res: 1.21, dres: 7.3, times: times() };
        assert_eq!(rec.to_control_string(), THIRD);
        let back = PjobControlRecord::parse(THIRD).unwrap();
        assert_eq!((back.n_left, back.n_tot, back.n_job, back.times), (901, 33, 2, times()));
        assert_eq!(back.sum, 39.93);
        assert_eq!(back.to_control_string(), THIRD, "writing what was read is the identity");
        // a NUL or a trailing blank after the text does not matter
        assert_eq!(PjobControlRecord::parse(&format!("{THIRD} \0\0")).unwrap(), back);
        // overflow stars, three-digit exponents without E, negative numbers
        assert_eq!(fmt_i(1_000_000_000_000_000, 15), "*".repeat(15));
        assert_eq!(fmt_e(1.0e100, 16, 8).trim(), "1.00000000+100");
        assert_eq!(fmt_e(-2.5e-7, 16, 8).trim(), "-2.50000000E-07");
        assert_eq!(fmt_e(f64::NAN, 16, 8), "*".repeat(16));
        assert_eq!(fmt_f(99.9, 7, 2), "  99.90");
        assert_eq!(fmt_f(123456.0, 7, 2), "*******");
        assert_eq!(fmt_f(f64::INFINITY, 7, 2), "*******");
        assert_eq!(parse_real("1.00000000+100"), Some(1.0e100));
        assert_eq!(parse_real("2.5D-3"), Some(2.5e-3));
        // errors: too few items, a word, an integer that is not
        assert!(matches!(PjobControlRecord::parse("1 2 3"), Err(EgsError::Fatal(_))));
        assert!(matches!(PjobControlRecord::parse(&THIRD.replace("901", "abc")), Err(EgsError::Fatal(_))));
        assert!(matches!(PjobControlRecord::parse(&THIRD.replace("2.5", "x")), Ok(_)));
        assert!(matches!(PjobControlRecord::parse(&THIRD.replace("3.99300000E+01", "zz")), Err(EgsError::Fatal(_))));
        assert_eq!(PjobAction::Update("a".into()).file_bytes().as_deref(), Some("a "));
        assert_eq!(PjobAction::Remove.file_bytes(), None);
    }

    /// A parallel run of three jobs (ncase 1000): the first calls create and
    /// then extend the control record exactly as the compiled upstream's did.
    #[test]
    fn three_jobs_start_like_the_compiled_upstream() {
        let cfg = |j| PjobConfig { n_parallel: 3, i_parallel: j, first_parallel: 1, is_uniform_run: false };
        let mut s1 = PjobJobState::default();
        let (r, a) = pjob_control(&cfg(1), &mut s1, 1000, 0, 0.0, 0.0, None).unwrap();
        assert_eq!((r.n_run, r.n_left, r.n_tot, r.res_dres), (33, 967, 0, Some((0.0, 99.9))));
        // the caller adds `t_first` (egs_date_and_time) to the first record
        let PjobAction::Create(text) = a else { panic!() };
        let first = PjobControlRecord::parse(&text).unwrap();
        assert_eq!(PjobControlRecord { times: times(), ..first }.to_control_string(), FIRST);
        assert_eq!(s1, PjobJobState { first_time: false, n_last: 33, n_max_parallel: 1 });
        // job 2, first call: reads the record, becomes the second job, takes a chunk
        let mut s2 = PjobJobState::default();
        let (r, a) = pjob_control(&cfg(2), &mut s2, 1000, 0, 0.0, 0.0, Some(FIRST)).unwrap();
        assert_eq!((r.n_run, r.n_left, r.n_tot), (33, 934, 0));
        assert_eq!(a, PjobAction::Update(SECOND.to_owned()));
        assert_eq!(s2.n_max_parallel, 2);
        // job 1, second call: 33 histories done with score 33 * 1.21 = 39.93
        let (r, a) = pjob_control(&cfg(1), &mut s1, 1000, 0, 39.93, 56.5653, Some(&format!("{SECOND} "))).unwrap();
        assert_eq!((r.n_run, r.n_left, r.n_tot), (33, 901, 33));
        let PjobAction::Update(text) = a else { panic!() };
        assert_eq!(text, THIRD, "same record as upstream");
        let (res, dres) = r.res_dres.unwrap();
        assert_eq!(res, 39.93 / 33.0);
        assert!((dres - 7.3).abs() < 0.05);
    }

    /// The chunk is clamped by what is left, `n_left` reaches 0, and the
    /// statistics branches: `n_tot <= 1` and a non-positive variance.
    #[test]
    fn chunks_clamp_and_statistics_branches() {
        let cfg = PjobConfig { n_parallel: 2, i_parallel: 2, first_parallel: 1, is_uniform_run: false };
        let rec = |n_left, n_tot, n_job, sum, sum2| PjobControlRecord { n_left, n_tot, n_job, sum, sum2, res: 0.0, dres: 99.9, times: [0; 8] }.to_control_string();
        let mut st = PjobJobState::default();
        st.first_time = false;
        st.n_last = 7;
        // 60 left, chunk is 100/2/10 = 5: ordinary
        let (r, _) = pjob_control(&cfg, &mut st, 100, 0, 1.0, 1.0, Some(&rec(60, 20, 2, 2.0, 5.0))).unwrap();
        assert_eq!((r.n_run, r.n_left, r.n_tot), (5, 55, 27));
        // 3 left: n_run clamps to 3, nothing left
        let (r, _) = pjob_control(&cfg, &mut st, 100, 0, 0.0, 0.0, Some(&rec(3, 20, 2, 2.0, 5.0))).unwrap();
        assert_eq!((r.n_run, r.n_left), (3, 0));
        // zero variance: dres = 99.9; res > 0
        let (r, _) = pjob_control(&cfg, &mut st, 100, 0, 0.0, 0.0, Some(&rec(0, 20, 2, 27.0, 27.0))).unwrap();
        assert_eq!(r.n_run, 0);
        assert_eq!(r.res_dres.unwrap().1, 99.9);
        // n_tot <= 1: res = 0, dres = 99.9
        let mut st = PjobJobState { first_time: false, n_last: 0, n_max_parallel: 5 };
        let (r, _) = pjob_control(&cfg, &mut st, 100, 0, 0.0, 0.0, Some(&rec(10, 0, 2, 0.0, 0.0))).unwrap();
        assert_eq!(r.res_dres, Some((0.0, 99.9)));
        assert_eq!(st.n_max_parallel, 5, "the maximum is kept");
        // a chunk larger than ncase on the very first call (n_parallel = 1, ncase small)
        let one = PjobConfig { n_parallel: 1, i_parallel: 1, first_parallel: 1, is_uniform_run: false };
        let (r, _) = pjob_control(&one, &mut PjobJobState::default(), 5, 0, 0.0, 0.0, None).unwrap();
        assert_eq!((r.n_run, r.n_left), (0, 5), "5/1/10 = 0 histories: finish at once");
        // a continuation from a previous run: n_tot > 1 gives statistics on the first record
        let (r, a) = pjob_control(&one, &mut PjobJobState::default(), 1000, 500, 620.5, 900.25, None).unwrap();
        let (res, dres) = r.res_dres.unwrap();
        assert_eq!(res, 620.5 / 500.0);
        assert!(dres > 0.0 && dres < 99.9);
        assert!(matches!(a, PjobAction::Create(_)));
    }

    /// The options without a lock file, and the refusal to run without text.
    #[test]
    fn shortcuts_and_errors() {
        let uniform = PjobConfig { n_parallel: 3, i_parallel: 2, first_parallel: 1, is_uniform_run: true };
        let mut st = PjobJobState::default();
        let (r, a) = pjob_control(&uniform, &mut st, 1000, 0, 0.0, 0.0, None).unwrap();
        assert_eq!((r.n_run, r.n_left, r.res_dres, a), (333, 0, None, PjobAction::None));
        let (r, _) = pjob_control(&uniform, &mut st, 1000, 0, 0.0, 0.0, None).unwrap();
        assert_eq!(r.n_run, 0);
        let serial = PjobConfig { n_parallel: 0, i_parallel: 0, first_parallel: 1, is_uniform_run: false };
        let (r, a) = pjob_control(&serial, &mut PjobJobState::default(), 1000, 7, 0.0, 0.0, None).unwrap();
        assert_eq!((r.n_run, r.n_tot, a), (1000, 7, PjobAction::None));
        // a job that is not the first needs the text
        let cfg = PjobConfig { n_parallel: 3, i_parallel: 2, first_parallel: 1, is_uniform_run: false };
        assert!(matches!(pjob_control(&cfg, &mut PjobJobState::default(), 1000, 0, 0.0, 0.0, None), Err(EgsError::Fatal(_))));
        assert!(matches!(pjob_control(&cfg, &mut PjobJobState::default(), 1000, 0, 0.0, 0.0, Some("garbage")), Err(EgsError::Fatal(_))));
        assert!(matches!(pjob_finish(&cfg, None), Err(EgsError::Fatal(_))));
        assert!(matches!(pjob_finish(&cfg, Some("1 2")), Err(EgsError::Fatal(_))));
    }

    /// `egs_pjob_finish`: the jobs leave one by one, the last removes the file;
    /// with `is_uniform_run` only job `n_parallel` completes.
    #[test]
    fn finish_decides_who_is_last() {
        let cfg = PjobConfig { n_parallel: 3, i_parallel: 2, first_parallel: 1, is_uniform_run: false };
        let rec = |n_job| PjobControlRecord { n_left: 0, n_tot: 1000, n_job, sum: 1259.02, sum2: 1841.5532, res: 1.25902, dres: 1.27, times: times() };
        let (r, a) = pjob_finish(&cfg, Some(&rec(3).to_control_string())).unwrap();
        assert_eq!((r.n_job, r.last, r.t_start), (2, false, None));
        assert_eq!(a, PjobAction::Update(rec(2).to_control_string()));
        let (r, a) = pjob_finish(&cfg, Some(&rec(1).to_control_string())).unwrap();
        assert_eq!((r.n_job, r.last, r.t_start, a), (0, true, Some(times()), PjobAction::Remove));
        for (j, last) in [(1, false), (2, false), (3, true)] {
            let u = PjobConfig { n_parallel: 3, i_parallel: j, first_parallel: 1, is_uniform_run: true };
            let (r, a) = pjob_finish(&u, None).unwrap();
            assert_eq!((r.n_job, r.last, a), (i32::from(!last), last, PjobAction::None));
        }
    }

    /// Phase 2: replay real parallel runs of the compiled upstream.
    /// `EGSNRC_PJOB_DIR` is a scenario directory made by the driver of
    /// `docs/egsnrc_phase2_differential.md` (processes of the compiled
    /// `egs_pjob_control` / `egs_pjob_finish`, scheduled one call at a time):
    /// `params.txt`, `log.txt` (one line per call: `kind job step n_run n_tot
    /// res dres n_max_parallel|n_job`) and `snap_NNN.bin` (the lock file after
    /// call `NNN`). `EGSNRC_PJOB_CFG=n_parallel,first_parallel,uniform(0|1)`.
    /// Every call is replayed with the inputs the Fortran driver passed; the
    /// returned `n_run`, `n_tot`, `res`, `dres`, `n_max_parallel`/`n_job` and the
    /// lock-file bytes must be **equal**. Skipped when the variable is unset.
    #[test]
    fn differential_parallel_run_against_compiled_egsnrc() {
        let Ok(dir) = std::env::var("EGSNRC_PJOB_DIR") else {
            eprintln!("EGSNRC_PJOB_DIR not set: differential test skipped");
            return;
        };
        let c: Vec<i32> = std::env::var("EGSNRC_PJOB_CFG").unwrap().split(',').map(|t| t.parse().unwrap()).collect();
        let (n_parallel, first_parallel, uniform) = (c[0], c[1], c[2] == 1);
        let params: Vec<f64> = std::fs::read_to_string(format!("{dir}/params.txt")).unwrap().split_whitespace().map(|t| t.parse().unwrap()).collect();
        let (ncase, ntot0, sum0, sum20) = (params[0] as i64, params[1] as i64, params[2], params[3]);
        let log = std::fs::read_to_string(format!("{dir}/log.txt")).unwrap();
        let mut file: Option<String> = None;
        // per job: state, the n_tot, sum, sum2 the Fortran driver will pass next
        let mut jobs: std::collections::BTreeMap<i32, (PjobJobState, i64, f64, f64)> = std::collections::BTreeMap::new();
        let (mut calls, mut bad) = (0, 0);
        for (seq, line) in log.lines().enumerate() {
            let t: Vec<&str> = line.split_whitespace().collect();
            let (kind, job, step): (i32, i32, i64) = (t[0].parse().unwrap(), t[1].parse().unwrap(), t[2].parse().unwrap());
            let cfg = PjobConfig { n_parallel, i_parallel: job, first_parallel, is_uniform_run: uniform };
            let snap = std::fs::read(format!("{dir}/snap_{:03}.bin", seq + 1)).unwrap();
            let e = jobs.entry(job).or_insert_with(|| {
                let own = job == first_parallel || n_parallel <= 0;
                (PjobJobState::default(), if own { ntot0 } else { 0 }, if own { sum0 } else { 0.0 }, if own { sum20 } else { 0.0 })
            });
            let mut ok = true;
            let action;
            if kind == 1 {
                let (r, a) = pjob_control(&cfg, &mut e.0, ncase, e.1, e.2, e.3, file.as_deref()).unwrap();
                let mut a = a;
                if let PjobAction::Create(text) = &a {
                    // the caller's `t_first`: `100 * i_parallel + i` in the driver
                    let rec = PjobControlRecord::parse(text).unwrap();
                    a = PjobAction::Create(PjobControlRecord { times: core::array::from_fn(|i| 100 * job + i as i32 + 1), ..rec }.to_control_string());
                }
                let (lr, lt): (i64, i64) = (t[3].parse().unwrap(), t[4].parse().unwrap());
                ok &= r.n_run == lr && r.n_tot == lt;
                if let Some((res, dres)) = r.res_dres {
                    ok &= res == t[5].parse::<f64>().unwrap() && dres == t[6].parse::<f64>().unwrap();
                }
                ok &= e.0.n_max_parallel == t[7].parse::<i32>().unwrap() || uniform || n_parallel <= 0;
                e.1 = r.n_tot;
                let rm = 1.0 + 0.1 * f64::from(job) + 0.01 * step as f64;
                e.2 = r.n_run as f64 * rm;
                e.3 = r.n_run as f64 * (rm * rm + 0.25);
                action = a;
            } else {
                let (r, a) = pjob_finish(&cfg, file.as_deref()).unwrap();
                ok &= r.n_job == t[7].parse::<i32>().unwrap();
                action = a;
            }
            match &action {
                PjobAction::Create(_) | PjobAction::Update(_) => file = action.file_bytes(),
                PjobAction::Remove => file = None,
                PjobAction::None => {}
            }
            let mine = file.clone().unwrap_or_default();
            ok &= mine.as_bytes() == snap.as_slice() || (uniform && snap.is_empty());
            calls += 1;
            if !ok {
                bad += 1;
                eprintln!("call {} (job {job} step {step}) differs:\n  port {mine:?}\n  ref  {:?}\n  log  {line}", seq + 1, String::from_utf8_lossy(&snap));
            }
        }
        eprintln!("parallel run: {calls} calls replayed, {bad} differ");
        assert_eq!(bad, 0);
    }
}
