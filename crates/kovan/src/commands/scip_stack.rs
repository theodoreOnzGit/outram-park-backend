//! Enough stack for `rust-analyzer scip` on every platform.
//!
//! `rust-analyzer scip` runs on the process's **main thread**. The LSP
//! server runs on a thread rust-analyzer creates with a large stack, but the
//! one-shot subcommands do not. The main thread gets 1 MB on Windows (the
//! `SizeOfStackReserve` in the executable's PE header) and the soft
//! `RLIMIT_STACK`, usually 8 MB, on Linux and macOS.
//!
//! **Measured 2026-10-22:** on the maintainer's Windows machine, `rust-analyzer scip`
//! over this workspace died with exit code `0xc00000fd`
//! (`STATUS_STACK_OVERFLOW`). Its log ended `thread 'main' (24656) has
//! overflowed its stack` after the workspace had loaded. The same run
//! completes on Linux with 8 MB, but nobody had measured how much of the
//! 8 MB it uses, so Linux gets the same headroom.
//!
//! What kovan does, so nobody has to `editbin` anything by hand:
//!
//! - **Windows:** copy the real `rust-analyzer.exe` (`rustup which`, not
//!   the rustup proxy, which only forwards to it) into
//!   `target/kovan-scip/` and raise the copy's `SizeOfStackReserve` to
//!   [`SCIP_STACK_BYTES`]. The toolchain's own file is never touched, and
//!   the copy is remade on every run, so a `rustup update` is picked up.
//! - **Unix:** raise the child's soft `RLIMIT_STACK` to [`SCIP_STACK_BYTES`]
//!   (capped at the hard limit) before it `exec`s, which sets the main
//!   thread's stack limit.
//!
//! The reserve is address space, not memory: pages are committed only as
//! the stack grows into them.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The main-thread stack `rust-analyzer scip` gets: 64 MB, eight times
/// Linux's default and 64 times Windows'.
pub const SCIP_STACK_BYTES: u64 = 64 << 20;

/// Raise the `SizeOfStackReserve` of the PE image in `image` to `reserve`
/// bytes, never lowering it. Returns the value it had.
///
/// The field sits at offset 72 of the optional header in both PE32 (a
/// `u32`) and PE32+ (a `u64`), per the Microsoft PE format specification.
/// The header checksum is left alone: the Windows loader checks it only for
/// drivers and boot-time DLLs, not for an ordinary executable.
pub fn set_pe_stack_reserve(image: &mut [u8], reserve: u64) -> Result<u64, String> {
    let get = |at: usize, n: usize| image.get(at..at + n).ok_or("PE image truncated");
    if get(0, 2)? != b"MZ" {
        return Err("not a PE image (no MZ header)".into());
    }
    let pe = u32::from_le_bytes(get(0x3C, 4)?.try_into().unwrap()) as usize;
    if get(pe, 4)? != b"PE\0\0" {
        return Err("not a PE image (no PE signature)".into());
    }
    // Signature (4) + COFF file header (20).
    let opt = pe + 24;
    let magic = u16::from_le_bytes(get(opt, 2)?.try_into().unwrap());
    let at = opt + 72;
    match magic {
        0x20b => {
            let old = u64::from_le_bytes(get(at, 8)?.try_into().unwrap());
            if old < reserve {
                image[at..at + 8].copy_from_slice(&reserve.to_le_bytes());
            }
            Ok(old)
        }
        0x10b => {
            let old = u32::from_le_bytes(get(at, 4)?.try_into().unwrap());
            let new = u32::try_from(reserve).map_err(|_| "reserve too large for PE32")?;
            if old < new {
                image[at..at + 4].copy_from_slice(&new.to_le_bytes());
            }
            Ok(u64::from(old))
        }
        m => Err(format!("unknown PE optional-header magic {m:#x}")),
    }
}

/// Copy the executable `source` to `dest` with its main-thread stack
/// reserve raised to [`SCIP_STACK_BYTES`] ([`set_pe_stack_reserve`]).
/// Returns the reserve the original had.
pub fn copy_with_big_stack(source: &Path, dest: &Path) -> Result<u64, String> {
    let mut image =
        std::fs::read(source).map_err(|e| format!("reading {}: {e}", source.display()))?;
    let old = set_pe_stack_reserve(&mut image, SCIP_STACK_BYTES)
        .map_err(|e| format!("{}: {e}", source.display()))?;
    std::fs::write(dest, &image).map_err(|e| format!("writing {}: {e}", dest.display()))?;
    Ok(old)
}

/// The real `rust-analyzer` executable for the toolchain `root` selects:
/// `rustup which rust-analyzer` run in `root` (so `rust-toolchain.toml`
/// applies), else whatever `rust-analyzer` is on `PATH`.
pub fn real_rust_analyzer(root: &Path) -> Option<PathBuf> {
    let via_rustup = Command::new("rustup")
        .args(["which", "rust-analyzer"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()))
        .filter(|p| p.is_file());
    via_rustup.or_else(|| which::which("rust-analyzer").ok())
}

/// The program to run for `rust-analyzer scip`. On Windows, a copy of the
/// real executable in `dir` with a [`SCIP_STACK_BYTES`] stack
/// ([`copy_with_big_stack`]); elsewhere, or when the copy cannot be made
/// (the reason is returned for the log), plain `rust-analyzer`.
pub fn scip_program(root: &Path, dir: &Path) -> (PathBuf, Option<String>) {
    let plain = PathBuf::from("rust-analyzer");
    if !cfg!(windows) {
        return (plain, None);
    }
    let Some(source) = real_rust_analyzer(root) else {
        return (
            plain,
            Some("rust-analyzer not found by rustup or on PATH".into()),
        );
    };
    let dest = dir.join("rust-analyzer-bigstack.exe");
    match copy_with_big_stack(&source, &dest) {
        Ok(old) => (
            dest,
            Some(format!(
                "running a copy of {} with a {} MB main-thread stack (it had {} KB)",
                source.display(),
                SCIP_STACK_BYTES >> 20,
                old >> 10
            )),
        ),
        Err(e) => (
            plain,
            Some(format!(
                "could not raise rust-analyzer's stack ({e}); running it as is"
            )),
        ),
    }
}

/// Give the child `cmd` a soft `RLIMIT_STACK` of [`SCIP_STACK_BYTES`],
/// capped at the hard limit, before it `exec`s (unix; a no-op elsewhere).
pub fn raise_child_stack_limit(cmd: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: the closure runs in the forked child before `exec` and calls
        // only `getrlimit`/`setrlimit`, which are async-signal-safe; it
        // allocates nothing and touches no lock.
        unsafe {
            cmd.pre_exec(|| {
                let mut lim = libc::rlimit {
                    rlim_cur: 0,
                    rlim_max: 0,
                };
                if libc::getrlimit(libc::RLIMIT_STACK, &mut lim) == 0 {
                    let want = SCIP_STACK_BYTES as libc::rlim_t;
                    let target = if lim.rlim_max == libc::RLIM_INFINITY {
                        want
                    } else {
                        want.min(lim.rlim_max)
                    };
                    if lim.rlim_cur != libc::RLIM_INFINITY && lim.rlim_cur < target {
                        lim.rlim_cur = target;
                        // Best effort: on failure the child keeps the old limit.
                        libc::setrlimit(libc::RLIMIT_STACK, &lim);
                    }
                }
                Ok(())
            });
        }
    }
    #[cfg(not(unix))]
    let _ = cmd;
}

/// When the log of a failed `rust-analyzer scip` run shows a stack overflow,
/// the sentence that says so, for the error message.
pub fn stack_overflow_note(log: &Path) -> Option<String> {
    let text = std::fs::read(log).ok()?;
    let tail = &text[text.len().saturating_sub(4096)..];
    String::from_utf8_lossy(tail).contains("overflowed its stack").then(|| {
        format!(
            "rust-analyzer overflowed its main-thread stack even with {} MB; raise `SCIP_STACK_BYTES` in kovan/src/commands/scip_stack.rs",
            SCIP_STACK_BYTES >> 20
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal PE header: MZ, `e_lfanew` = 0x80, the PE signature, and an
    /// optional header with `magic` and a stack reserve of 1 MB.
    fn fake_pe(magic: u16) -> Vec<u8> {
        let mut b = vec![0u8; 0x80 + 24 + 112];
        b[0..2].copy_from_slice(b"MZ");
        b[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        b[0x80..0x84].copy_from_slice(b"PE\0\0");
        let opt = 0x80 + 24;
        b[opt..opt + 2].copy_from_slice(&magic.to_le_bytes());
        if magic == 0x20b {
            b[opt + 72..opt + 80].copy_from_slice(&(1u64 << 20).to_le_bytes());
        } else {
            b[opt + 72..opt + 76].copy_from_slice(&(1u32 << 20).to_le_bytes());
        }
        b
    }

    #[test]
    fn pe32_plus_reserve_is_raised_and_never_lowered() {
        let mut b = fake_pe(0x20b);
        assert_eq!(set_pe_stack_reserve(&mut b, SCIP_STACK_BYTES), Ok(1 << 20));
        let at = 0x80 + 24 + 72;
        assert_eq!(
            u64::from_le_bytes(b[at..at + 8].try_into().unwrap()),
            SCIP_STACK_BYTES
        );
        // A smaller request leaves the larger value in place.
        assert_eq!(set_pe_stack_reserve(&mut b, 1 << 20), Ok(SCIP_STACK_BYTES));
        assert_eq!(
            u64::from_le_bytes(b[at..at + 8].try_into().unwrap()),
            SCIP_STACK_BYTES
        );
    }

    #[test]
    fn pe32_reserve_is_raised() {
        let mut b = fake_pe(0x10b);
        assert_eq!(set_pe_stack_reserve(&mut b, SCIP_STACK_BYTES), Ok(1 << 20));
        let at = 0x80 + 24 + 72;
        assert_eq!(
            u64::from(u32::from_le_bytes(b[at..at + 4].try_into().unwrap())),
            SCIP_STACK_BYTES
        );
        // Nothing else changed: the bytes after the field are still zero.
        assert!(b[at + 4..].iter().all(|&x| x == 0));
    }

    #[test]
    fn non_pe_input_is_refused() {
        assert!(set_pe_stack_reserve(&mut b"\x7fELF....".to_vec(), SCIP_STACK_BYTES).is_err());
        assert!(set_pe_stack_reserve(&mut b"MZ".to_vec(), SCIP_STACK_BYTES).is_err());
        let mut bad_magic = fake_pe(0x999);
        assert!(set_pe_stack_reserve(&mut bad_magic, SCIP_STACK_BYTES).is_err());
    }

    #[test]
    fn copy_with_big_stack_writes_a_patched_copy_and_leaves_the_source() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("ra.exe");
        let dst = dir.path().join("ra-big.exe");
        std::fs::write(&src, fake_pe(0x20b)).unwrap();
        assert_eq!(copy_with_big_stack(&src, &dst), Ok(1 << 20));
        assert_eq!(std::fs::read(&src).unwrap(), fake_pe(0x20b));
        let mut expect = fake_pe(0x20b);
        set_pe_stack_reserve(&mut expect, SCIP_STACK_BYTES).unwrap();
        assert_eq!(std::fs::read(&dst).unwrap(), expect);
    }

    #[test]
    fn scip_program_off_windows_is_plain_rust_analyzer() {
        let dir = tempfile::tempdir().unwrap();
        let (prog, note) = scip_program(dir.path(), dir.path());
        if cfg!(windows) {
            assert!(note.is_some());
        } else {
            assert_eq!(prog, PathBuf::from("rust-analyzer"));
            assert_eq!(note, None);
        }
    }

    #[test]
    fn real_rust_analyzer_is_a_file_when_found() {
        if let Some(p) = real_rust_analyzer(Path::new(env!("CARGO_MANIFEST_DIR"))) {
            assert!(p.is_file(), "{}", p.display());
        }
    }

    /// The child sees the raised soft limit (or the hard limit, when that
    /// is lower). `ulimit -s` reports KiB.
    #[cfg(unix)]
    #[test]
    fn child_gets_the_raised_stack_limit() {
        let sh = |script: &str, raise: bool| {
            let mut c = Command::new("sh");
            c.args(["-c", script]);
            if raise {
                raise_child_stack_limit(&mut c);
            }
            String::from_utf8(c.output().unwrap().stdout)
                .unwrap()
                .trim()
                .to_string()
        };
        let before = sh("ulimit -s", false);
        let hard = sh("ulimit -Hs", false);
        let got = sh("ulimit -s", true);
        let want_kib = SCIP_STACK_BYTES >> 10;
        if before == "unlimited" {
            assert_eq!(got, "unlimited");
        } else if hard == "unlimited" || hard.parse::<u64>().unwrap() >= want_kib {
            let before: u64 = before.parse().unwrap();
            assert_eq!(got.parse::<u64>().unwrap(), want_kib.max(before));
        } else {
            assert_eq!(got, hard);
        }
    }

    #[test]
    fn overflow_in_the_log_is_named() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("ra.log");
        std::fs::write(
            &log,
            "loading\n\nthread 'main' (24656) has overflowed its stack\n",
        )
        .unwrap();
        assert!(stack_overflow_note(&log).unwrap().contains("64 MB"));
        std::fs::write(&log, "error: something else\n").unwrap();
        assert_eq!(stack_overflow_note(&log), None);
    }
}
