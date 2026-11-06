//! **The "Signing key" window's state machine** (GitHub #1020): the key
//! session's lock state and auto-lock, and moving a key between machines
//! by **export** and **import** of its encrypted file. No egui; the drawing
//! is `app/key_window.rs`. The checks and the file handling are
//! `kovan_common::review::signing::{keystore, session}`.
//!
//! ```text
//!  open ──> List (worker: keystore list + kovan_root.toml)
//!            v
//!  🔓 key (fingerprint, locks in …) [Lock now]   or   🔒 locked
//!  auto-lock after N minutes (saved in kovan's config folder)
//!  keys: reviewer, key, fingerprint, registered?
//!    Export… ── save picker ──> Export (worker: Keystore::export) ──> notice + fingerprint
//!    Import… ── file picker + passphrase ──> Import (worker: Keystore::import against
//!               kovan_root.toml; the key is held for the session) ──> List again
//! ```
//!
//! **kovan never sends the file anywhere**: export writes it where the user
//! picked, and the user carries it (the maintainer uses secure e-mail). The
//! passphrase is unchanged by both; the import's passphrase is zeroised in
//! its worker and when the panel drops. **No lag**: every file and KDF step
//! runs on a [`Worker`]; [`KeyPanel::poll`] never blocks.

use std::path::{Path, PathBuf};
use std::time::Duration;

use kovan_common::review::root::ReviewRoot;
use kovan_common::review::signing::keystore::{ExportedKey, ImportedKey, Keystore};
use kovan_common::review::signing::session::{SessionStatus, SharedKeySession, DEFAULT_AUTO_LOCK};
use zeroize::Zeroize;

use super::flow::{find_keys, KeyInfo, Worker};
use super::ROOT_FILE;

/// The shortest and longest auto-lock the window offers, in minutes.
pub const AUTO_LOCK_MINUTES: std::ops::RangeInclusive<u32> = 1..=480;

/// The settings file's contents (`key_session.toml` in kovan's config
/// folder).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KeySessionSettings {
    /// Lock the key after this many minutes without signing.
    pub auto_lock_minutes: u32,
}

impl Default for KeySessionSettings {
    fn default() -> Self {
        KeySessionSettings { auto_lock_minutes: (DEFAULT_AUTO_LOCK.as_secs() / 60) as u32 }
    }
}

impl KeySessionSettings {
    /// The timeout, clamped to [`AUTO_LOCK_MINUTES`].
    pub fn auto_lock(&self) -> Duration {
        let m = self.auto_lock_minutes.clamp(*AUTO_LOCK_MINUTES.start(), *AUTO_LOCK_MINUTES.end());
        Duration::from_secs(u64::from(m) * 60)
    }

    /// Read `path`; the defaults when it is missing or does not parse.
    pub fn read(path: &Path) -> KeySessionSettings {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| toml::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// Write `path` (creating its folder).
    pub fn write(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let text = toml::to_string(self).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// `<kovan config dir>/key_session.toml`; `None` under `cargo test`, so a
/// test never reads or writes the user's setting.
pub fn settings_file() -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    directories::ProjectDirs::from("org", "OUTRAM PARK", "kovan").map(|d| d.config_dir().join("key_session.toml"))
}

/// A file name for an exported key: the keystore's own name for it.
pub fn export_file_name(k: &KeyInfo) -> String {
    k.path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| format!("{}.kovankey", k.key))
}

/// The status line for the session: 🔓 with the key and the minutes left,
/// or 🔒.
pub fn status_line(status: Option<&SessionStatus>) -> String {
    match status {
        Some(s) => format!(
            "\u{1F513} {} (key {}) unlocked, locks in {} min",
            s.reviewer,
            s.key,
            s.locks_in.as_secs().div_ceil(60)
        ),
        None => "\u{1F512} Signing key locked".to_string(),
    }
}

/// "Lock now" without blocking the UI thread: `Some(held)` when done,
/// `None` while a signing job holds the session (try again next frame).
pub fn try_lock(session: &SharedKeySession) -> Option<bool> {
    session.try_write().ok().map(|mut s| s.lock())
}

enum Job {
    List(Worker<Result<Vec<KeyInfo>, String>>),
    Export(Worker<Result<ExportedKey, String>>),
    Import(Worker<Result<ImportedKey, String>>),
}

/// The window's whole state (module doc).
pub struct KeyPanel {
    root: PathBuf,
    keystore: Keystore,
    session: SharedKeySession,
    /// Where the auto-lock setting is saved (`None`: not saved, as in tests).
    settings: Option<PathBuf>,
    pub keys: Vec<KeyInfo>,
    /// Index into `keys` of the key to export.
    pub chosen: usize,
    /// The file picked to import.
    pub import_source: Option<PathBuf>,
    /// The imported key's passphrase (zeroised after use and on drop).
    pub import_passphrase: String,
    pub auto_lock_minutes: u32,
    /// Plain-words outcomes (exported to …, imported …, with fingerprints).
    pub notices: Vec<String>,
    pub error: Option<String>,
    job: Option<Job>,
    /// A key was imported since the host last asked ([`Self::take_imported`]).
    imported: bool,
    /// "Lock now" or a new auto-lock pressed while a signing job held the
    /// session: applied by [`Self::poll`] as soon as it is free.
    lock_pending: bool,
    auto_lock_pending: Option<Duration>,
}

impl Drop for KeyPanel {
    fn drop(&mut self) {
        self.import_passphrase.zeroize();
    }
}

impl KeyPanel {
    /// Open the window over workspace `root` (its `kovan_root.toml` is the
    /// registry an import is checked against), reading `keystore` and the
    /// app's `session`; `settings` is where the auto-lock is saved.
    /// Listing starts on a worker.
    pub fn new(root: PathBuf, keystore: Keystore, session: SharedKeySession, settings: Option<PathBuf>) -> KeyPanel {
        let auto_lock_minutes = session
            .try_read()
            .map(|s| (s.auto_lock().as_secs() / 60) as u32)
            .unwrap_or(KeySessionSettings::default().auto_lock_minutes);
        let mut p = KeyPanel {
            root,
            keystore,
            session,
            settings,
            keys: Vec::new(),
            chosen: 0,
            import_source: None,
            import_passphrase: String::new(),
            auto_lock_minutes,
            notices: Vec::new(),
            error: None,
            job: None,
            imported: false,
            lock_pending: false,
            auto_lock_pending: None,
        };
        p.list();
        p
    }

    /// A job is running.
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    /// The keystore folder (shown in the window).
    pub fn keystore_dir(&self) -> &Path {
        self.keystore.dir()
    }

    /// The key chosen for export.
    pub fn key(&self) -> Option<&KeyInfo> {
        self.keys.get(self.chosen)
    }

    fn list(&mut self) {
        let (root, ks) = (self.root.clone(), self.keystore.clone());
        self.job = Some(Job::List(Worker::spawn(move || find_keys(&root, &ks).map(|f| f.keys))));
    }

    /// Take a finished job's result, and apply a pending lock or auto-lock;
    /// never blocks.
    pub fn poll(&mut self) {
        if self.lock_pending && try_lock(&self.session).is_some() {
            self.lock_pending = false;
        }
        if let Some(d) = self.auto_lock_pending {
            if let Ok(mut s) = self.session.try_write() {
                s.set_auto_lock(d);
                self.auto_lock_pending = None;
            }
        }
        let Some(job) = self.job.take() else { return };
        match job {
            Job::List(w) => match w.try_take() {
                None => self.job = Some(Job::List(w)),
                Some(Ok(keys)) => {
                    self.keys = keys;
                    if self.chosen >= self.keys.len() {
                        self.chosen = 0;
                    }
                }
                Some(Err(e)) => self.error = Some(e),
            },
            Job::Export(w) => match w.try_take() {
                None => self.job = Some(Job::Export(w)),
                Some(Ok(x)) => self.notices.push(format!(
                    "Exported key {} of {} to {}. Fingerprint {}: check the same fingerprint \
                     after importing it on the other machine. The file is still encrypted with \
                     your passphrase; kovan sent it nowhere.",
                    x.key,
                    x.reviewer,
                    x.path.display(),
                    x.fingerprint
                )),
                Some(Err(e)) => self.error = Some(e),
            },
            Job::Import(w) => match w.try_take() {
                None => self.job = Some(Job::Import(w)),
                Some(Ok(i)) => {
                    self.notices.push(if i.already_present {
                        format!(
                            "Key {} of {} is already installed here (fingerprint {}); nothing \
                             was written. It is unlocked for this session.",
                            i.key, i.reviewer, i.fingerprint
                        )
                    } else {
                        format!(
                            "Imported key {} of {} to {} (fingerprint {}: the same as on the \
                             machine it came from). It is already in kovan_root.toml, so there \
                             is nothing to endorse, and it is unlocked for this session.",
                            i.key,
                            i.reviewer,
                            i.path.display(),
                            i.fingerprint
                        )
                    });
                    self.import_source = None;
                    self.imported = true;
                    self.list();
                }
                Some(Err(e)) => self.error = Some(e),
            },
        }
    }

    /// The session's status for the indicator (`None`: locked, or the
    /// session is being written this instant).
    pub fn status(&self) -> Option<SessionStatus> {
        self.session.try_read().ok().and_then(|s| s.status())
    }

    /// "Lock now": at once, or (a signing job holds the session) at the
    /// next [`Self::poll`]. Whether a key was dropped now.
    pub fn lock_now(&mut self) -> bool {
        match try_lock(&self.session) {
            Some(held) => held,
            None => {
                self.lock_pending = true;
                false
            }
        }
    }

    /// Set the auto-lock to `minutes` (clamped to [`AUTO_LOCK_MINUTES`]):
    /// applied to the session now and saved for the next start.
    pub fn set_auto_lock_minutes(&mut self, minutes: u32) {
        let settings = KeySessionSettings { auto_lock_minutes: minutes };
        let d = settings.auto_lock();
        self.auto_lock_minutes = (d.as_secs() / 60) as u32;
        match self.session.try_write() {
            Ok(mut s) => s.set_auto_lock(d),
            Err(_) => self.auto_lock_pending = Some(d),
        }
        if let Some(path) = &self.settings {
            let saved = KeySessionSettings { auto_lock_minutes: self.auto_lock_minutes };
            if let Err(e) = saved.write(path) {
                self.error = Some(format!("The auto-lock was not saved: {e}"));
            }
        }
    }

    /// Export the chosen key to `dest` (worker). Refused while a job runs
    /// or there is no key.
    pub fn export_to(&mut self, dest: PathBuf) -> bool {
        let Some(k) = self.key().cloned() else { return false };
        if self.job.is_some() {
            return false;
        }
        let ks = self.keystore.clone();
        self.error = None;
        self.job = Some(Job::Export(Worker::spawn(move || {
            ks.export(&k.reviewer, &k.key, &dest).map_err(|e| e.to_string())
        })));
        true
    }

    /// Whether "Import" is allowed: a file picked, a passphrase typed, no
    /// job running.
    pub fn can_import(&self) -> bool {
        self.job.is_none() && self.import_source.is_some() && !self.import_passphrase.is_empty()
    }

    /// Import the picked file (worker): checked against this workspace's
    /// `kovan_root.toml`, installed in the keystore, and the key held for
    /// the session.
    pub fn import(&mut self) -> bool {
        if !self.can_import() {
            return false;
        }
        let Some(source) = self.import_source.clone() else { return false };
        let mut pass = std::mem::take(&mut self.import_passphrase);
        let (root, ks, session) = (self.root.clone(), self.keystore.clone(), self.session.clone());
        self.error = None;
        self.job = Some(Job::Import(Worker::spawn(move || {
            let registry = std::fs::read_to_string(root.join(ROOT_FILE))
                .map_err(|e| format!("{ROOT_FILE}: {e}"))
                .and_then(|t| ReviewRoot::parse(&t).map_err(|e| format!("{ROOT_FILE}: {e}")));
            let done = registry.and_then(|r| ks.import(&source, &pass, &r).map_err(|e| e.to_string()));
            pass.zeroize();
            let (imported, key) = done?;
            session.write().unwrap_or_else(|p| p.into_inner()).hold(key);
            Ok(imported)
        })));
        true
    }

    /// Whether a key was imported since the last call (the host then
    /// reloads an open stamp dialog's keys).
    pub fn take_imported(&mut self) -> bool {
        std::mem::take(&mut self.imported)
    }

    /// Poll until no job runs (tests only; the UI never waits).
    #[cfg(test)]
    pub fn wait(&mut self) {
        let start = std::time::Instant::now();
        while self.busy() {
            assert!(start.elapsed() < Duration::from_secs(120), "key panel job did not finish");
            std::thread::sleep(Duration::from_millis(5));
            self.poll();
        }
    }
}

#[cfg(test)]
mod tests {
    //! Methodology: the throwaway git repository of `stamping::tests` (a
    //! registered founder key, real argon2id + AES-GCM + ed25519) stands
    //! for one workspace; two temporary keystores stand for two machines'
    //! config folders. Nothing reads the real keystore or settings.

    use std::time::Duration;

    use kovan_common::review::signing::keystore::{generate, Keystore};
    use kovan_common::review::signing::session::{KeySession, SessionClock, DEFAULT_AUTO_LOCK};
    use kovan_common::review::state::StateKind;

    use super::super::flow::{Purpose, StampFlow, Step, Target, WizardForm};
    use super::super::tests::{clean, founder_key, today, Repo, BY, LIB, PASS};
    use super::super::{StampContext, ROOT_FILE};
    use super::*;

    /// Export on machine A, import on machine B, stamp on B. Pass: the
    /// export notice and the import notice carry the same fingerprint as
    /// A's key; B's keystore then lists the key as registered; the import
    /// leaves it unlocked for the session, so B's stamp dialog signs
    /// without a passphrase; the stamp is valid at rung 3 against the
    /// registry entry A's key already had (B registers nothing:
    /// `kovan_root.toml` is not among the files written).
    ///
    /// Result (2026-10-22): passes.
    #[test]
    fn export_on_a_import_on_b_and_stamp_there() {
        let r = Repo::new();
        let store_a = tempfile::tempdir().unwrap();
        let store_b = tempfile::tempdir().unwrap();
        let carry = tempfile::tempdir().unwrap();
        founder_key(&r, store_a.path());

        let mut a = KeyPanel::new(r.path().into(), Keystore::at(store_a.path()), SharedKeySession::default(), None);
        a.wait();
        assert_eq!(a.keys.len(), 1);
        let fp = a.keys[0].fingerprint.clone();
        assert!(fp.starts_with("SHA256:") && a.keys[0].registered);
        assert_eq!(a.keystore_dir(), store_a.path());
        let name = export_file_name(a.key().unwrap());
        assert!(name.ends_with("--k1.kovankey"), "{name}");
        let dest = carry.path().join(&name);
        assert!(a.export_to(dest.clone()));
        assert!(!a.export_to(dest.clone()), "one job at a time");
        a.wait();
        assert!(a.error.is_none(), "{:?}", a.error);
        assert!(a.notices[0].contains(&fp) && dest.exists(), "{:?}", a.notices);
        assert!(a.export_to(dest.clone()));
        a.wait();
        assert!(a.error.as_deref().unwrap().contains("already exists"), "never overwritten");

        let clock = SessionClock::manual();
        let session = KeySession::shared(DEFAULT_AUTO_LOCK, clock);
        let mut b = KeyPanel::new(r.path().into(), Keystore::at(store_b.path()), session.clone(), None);
        b.wait();
        assert!(b.keys.is_empty() && b.status().is_none());
        assert!(!b.can_import() && !b.import());
        b.import_source = Some(dest);
        b.import_passphrase = PASS.into();
        assert!(b.can_import());
        assert!(b.import());
        assert!(b.import_passphrase.is_empty(), "taken out of the form");
        b.wait();
        assert!(b.error.is_none(), "{:?}", b.error);
        assert!(b.notices[0].contains(&fp), "{:?}", b.notices);
        assert!(b.take_imported() && !b.take_imported());
        assert_eq!(b.keys.len(), 1);
        assert_eq!((b.keys[0].fingerprint.as_str(), b.keys[0].registered), (fp.as_str(), true));
        assert_eq!(b.status().unwrap().fingerprint, fp);

        let target = Target { function: format!("{LIB}::leaf"), name: "leaf".into() };
        let mut f = StampFlow::new(r.path().into(), Keystore::at(store_b.path()), Purpose::Stamp, target)
            .with_session(session);
        f.wait();
        assert_eq!(f.step, Step::Wizard, "{:?}", f.error);
        let w = f.wizard.as_mut().unwrap();
        *w = WizardForm::new(StampContext { answers: clean(), ..w.ctx.clone() });
        w.choose_reason(Some("plumbing"));
        assert!(f.can_sign(), "unlocked by the import");
        assert!(f.sign());
        f.wait();
        assert!(matches!(f.step, Step::Done(_)), "{:?} {:?}", f.step, f.error);
        assert!(!f.wrote.contains(ROOT_FILE), "nothing registered on machine B");
        r.commit("stamp leaf on machine B");
        let s = r.state("leaf").unwrap();
        assert_eq!((s.state, s.rung), (Some(StateKind::Valid), 3), "{}", s.reason);
    }

    /// The window shows the import refusals: a key the registry does not
    /// list (with what to do), a clash with a different key file of the
    /// same name, and a workspace without `kovan_root.toml`. None leaves a
    /// key unlocked or writes a key file.
    #[test]
    fn import_refusals_are_shown() {
        let r = Repo::new();
        let store_a = tempfile::tempdir().unwrap();
        let store_b = tempfile::tempdir().unwrap();
        let carry = tempfile::tempdir().unwrap();
        founder_key(&r, store_a.path());
        let ks_a = Keystore::at(store_a.path());
        let registered = carry.path().join("k1.kovankey");
        ks_a.export(BY, "k1", &registered).unwrap();
        let (stranger, _) = generate("github:stranger", "s1", &today(), PASS).unwrap();
        let unregistered = carry.path().join("s1.kovankey");
        std::fs::write(&unregistered, stranger.to_toml().unwrap()).unwrap();

        let session = SharedKeySession::default();
        let ks_b = Keystore::at(store_b.path());
        let mut b = KeyPanel::new(r.path().into(), ks_b.clone(), session.clone(), None);
        b.wait();
        let try_import = |b: &mut KeyPanel, src: &Path| {
            b.import_source = Some(src.to_path_buf());
            b.import_passphrase = PASS.into();
            assert!(b.import());
            b.wait();
            b.error.take().expect("refused")
        };
        let e = try_import(&mut b, &unregistered);
        assert!(e.contains("not in this workspace's kovan_root.toml"), "{e}");
        assert!(ks_b.list().unwrap().is_empty());

        let (own, _) = generate(BY, "k1", &today(), PASS).unwrap();
        ks_b.save(&own).unwrap();
        let e = try_import(&mut b, &registered);
        assert!(e.contains("nothing was overwritten"), "{e}");
        assert_eq!(ks_b.load(BY, "k1").unwrap(), own);

        let bare = tempfile::tempdir().unwrap();
        let mut c = KeyPanel::new(bare.path().into(), Keystore::at(bare.path().join("keys")), session.clone(), None);
        c.wait();
        let e = try_import(&mut c, &registered);
        assert!(e.contains(ROOT_FILE), "{e}");
        assert!(session.read().unwrap().status().is_none() && !b.take_imported());
    }

    /// The auto-lock setting (clamped, saved, read back, defaults when
    /// missing or broken), the status line, Lock now (at once, and deferred
    /// while a signing job holds the session), and an auto-lock change
    /// deferred the same way.
    #[test]
    fn settings_status_and_lock_now() {
        assert_eq!(KeySessionSettings::default().auto_lock(), DEFAULT_AUTO_LOCK);
        assert_eq!(KeySessionSettings { auto_lock_minutes: 0 }.auto_lock(), Duration::from_secs(60));
        assert_eq!(KeySessionSettings { auto_lock_minutes: 10_000 }.auto_lock(), Duration::from_secs(480 * 60));
        assert!(settings_file().is_none(), "never the user's file under test");
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("cfg").join("key_session.toml");
        assert_eq!(KeySessionSettings::read(&path), KeySessionSettings::default());
        KeySessionSettings { auto_lock_minutes: 12 }.write(&path).unwrap();
        assert_eq!(KeySessionSettings::read(&path).auto_lock_minutes, 12);
        std::fs::write(&path, "auto_lock_minutes = \"x\"").unwrap();
        assert_eq!(KeySessionSettings::read(&path), KeySessionSettings::default());
        assert!(KeySessionSettings::default().write(d.path()).is_err(), "a folder is not a file");

        let r = Repo::new();
        let store = tempfile::tempdir().unwrap();
        let key = founder_key(&r, store.path());
        let session = KeySession::shared(DEFAULT_AUTO_LOCK, SessionClock::manual());
        let mut p = KeyPanel::new(r.path().into(), Keystore::at(store.path()), session.clone(), Some(path.clone()));
        p.wait();
        assert_eq!(p.auto_lock_minutes, 30);
        assert!(status_line(p.status().as_ref()).contains("locked"));
        p.set_auto_lock_minutes(0);
        assert_eq!(p.auto_lock_minutes, 1);
        assert_eq!(session.read().unwrap().auto_lock(), Duration::from_secs(60));
        assert_eq!(KeySessionSettings::read(&path).auto_lock_minutes, 1);

        session.write().unwrap().hold(key);
        let line = status_line(p.status().as_ref());
        assert!(line.contains("unlocked") && line.contains(BY) && line.contains("1 min"), "{line}");
        assert!(p.lock_now());
        assert!(!p.lock_now(), "nothing held");

        // A signing job holds the session: both wait for the next poll.
        let (_, again) = generate(BY, "k9", &today(), PASS).unwrap();
        session.write().unwrap().hold(again);
        {
            let _held = session.write().unwrap();
            assert!(!p.lock_now());
            p.set_auto_lock_minutes(5);
            p.poll();
        }
        assert!(session.read().unwrap().status().is_some(), "not yet locked");
        p.poll();
        assert!(session.read().unwrap().status().is_none(), "locked at the next poll");
        assert_eq!(session.read().unwrap().auto_lock(), Duration::from_secs(300));
        let mut bad = KeyPanel::new(r.path().into(), Keystore::at(store.path()), session, Some(d.path().into()));
        bad.wait();
        bad.set_auto_lock_minutes(3);
        assert!(bad.error.as_deref().unwrap().contains("not saved"));
    }
}
