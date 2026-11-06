//! **The key session**: unlock the signing key once, keep it in memory
//! while kovan is open, lock it after inactivity (GitHub #1020; maintainer
//! decision 2026-10-22, from the #1018 dogfooding: "I'm still being asked
//! for keys"). **Native only**, like [`super::keystore`].
//!
//! ```text
//!   first signing:  KeyFile::unlock(passphrase) ──> KeySession holds the UnlockedKey
//!   later signings: the held key signs; no passphrase asked
//!   idle for auto_lock (default 30 min), "Lock now", or quit ──> dropped (zeroised)
//! ```
//!
//! **What "inactivity" means here.** The clock restarts at every unlock and
//! every signature made with the held key; the key locks once
//! [`KeySession::auto_lock`] passes without either. Moving the mouse does
//! not keep it unlocked: only using the key does.
//!
//! **Never on disk.** The session holds one [`UnlockedKey`] in memory, and
//! dropping it (lock, auto-lock, replacement by another key, or the app
//! exiting) zeroises the secret (`ed25519-dalek`'s `ZeroizeOnDrop`). The
//! passphrase is never stored: it is used for one unlock and dropped by the
//! caller.
//!
//! **Shared across threads** as [`SharedKeySession`]
//! (`Arc<RwLock<KeySession>>`, the workspace rule for shared mutable
//! state): signing runs on worker threads, the indicator reads it from the
//! UI thread with `try_read`. The slow part (argon2) runs **outside** the
//! lock ([`ensure_unlocked`]); only the ed25519 signature runs under it.
//!
//! **The clock is injectable** ([`SessionClock::Manual`]), so the auto-lock
//! is tested without sleeping.

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use super::keystore::{KeyFile, KeystoreError, UnlockedKey};

/// The auto-lock timeout when none is configured: 30 minutes (#1020).
pub const DEFAULT_AUTO_LOCK: Duration = Duration::from_secs(30 * 60);

/// Where a [`KeySession`] reads the time. An enum, not a trait object
/// (workspace rule): the real clock, or one a test moves by hand.
#[derive(Debug, Clone, Default)]
pub enum SessionClock {
    /// [`Instant::now`].
    #[default]
    System,
    /// A fixed instant, moved only by [`SessionClock::advance`].
    Manual(Arc<RwLock<Instant>>),
}

impl SessionClock {
    /// A manual clock starting now.
    pub fn manual() -> SessionClock {
        SessionClock::Manual(Arc::new(RwLock::new(Instant::now())))
    }

    /// The current instant.
    pub fn now(&self) -> Instant {
        match self {
            SessionClock::System => Instant::now(),
            SessionClock::Manual(t) => *t.read().unwrap_or_else(|p| p.into_inner()),
        }
    }

    /// Move a manual clock forward by `by`; the system clock ignores it.
    pub fn advance(&self, by: Duration) {
        if let SessionClock::Manual(t) = self {
            let mut t = t.write().unwrap_or_else(|p| p.into_inner());
            *t += by;
        }
    }
}

/// The held key and when it was last used.
struct Held {
    key: UnlockedKey,
    last_used: Instant,
}

/// What the indicator shows while a key is unlocked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionStatus {
    pub reviewer: String,
    pub key: String,
    pub fingerprint: String,
    /// Time left before the auto-lock.
    pub locks_in: Duration,
}

/// The session: at most one unlocked key, and the auto-lock timeout.
pub struct KeySession {
    clock: SessionClock,
    auto_lock: Duration,
    held: Option<Held>,
}

/// A session shared between the UI thread and the signing workers.
pub type SharedKeySession = Arc<RwLock<KeySession>>;

impl Default for KeySession {
    fn default() -> Self {
        KeySession::new(DEFAULT_AUTO_LOCK, SessionClock::System)
    }
}

impl std::fmt::Debug for KeySession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeySession")
            .field("auto_lock", &self.auto_lock)
            .field("held", &self.held.as_ref().map(|h| (h.key.reviewer(), h.key.key_id())))
            .finish()
    }
}

impl KeySession {
    /// A locked session that auto-locks after `auto_lock` without use.
    pub fn new(auto_lock: Duration, clock: SessionClock) -> KeySession {
        KeySession { clock, auto_lock, held: None }
    }

    /// [`Self::new`] wrapped for sharing.
    pub fn shared(auto_lock: Duration, clock: SessionClock) -> SharedKeySession {
        Arc::new(RwLock::new(KeySession::new(auto_lock, clock)))
    }

    /// The auto-lock timeout.
    pub fn auto_lock(&self) -> Duration {
        self.auto_lock
    }

    /// Change the auto-lock timeout; it applies to the key held now too.
    pub fn set_auto_lock(&mut self, auto_lock: Duration) {
        self.auto_lock = auto_lock;
    }

    fn live(&self) -> Option<&Held> {
        self.held
            .as_ref()
            .filter(|h| self.clock.now().saturating_duration_since(h.last_used) < self.auto_lock)
    }

    /// Drop the key if it has been idle for the timeout; whether it was
    /// dropped now. Called every frame by the app, and before every use.
    pub fn expire_if_idle(&mut self) -> bool {
        if self.held.is_some() && self.live().is_none() {
            self.held = None;
            return true;
        }
        false
    }

    /// "Lock now" (and quit): drop the key; whether one was held.
    pub fn lock(&mut self) -> bool {
        self.held.take().is_some()
    }

    /// Whether `reviewer`'s key `key` is unlocked and not idle past the
    /// timeout (read-only: the UI asks it every frame).
    pub fn holds(&self, reviewer: &str, key: &str) -> bool {
        self.live()
            .is_some_and(|h| h.key.reviewer() == reviewer && h.key.key_id() == key)
    }

    /// What the indicator shows; `None` while locked (or idle past the
    /// timeout).
    pub fn status(&self) -> Option<SessionStatus> {
        let h = self.live()?;
        let idle = self.clock.now().saturating_duration_since(h.last_used);
        Some(SessionStatus {
            reviewer: h.key.reviewer().to_string(),
            key: h.key.key_id().to_string(),
            fingerprint: h.key.fingerprint(),
            locks_in: self.auto_lock.saturating_sub(idle),
        })
    }

    /// Hold `key` from now (a freshly generated, imported or unlocked
    /// key), replacing (and zeroising) any other.
    pub fn hold(&mut self, key: UnlockedKey) {
        self.held = Some(Held { key, last_used: self.clock.now() });
    }

    /// Run `f` with the held key when it is `file`'s (same reviewer, key id
    /// and public key) and not idle past the timeout; restarts the idle
    /// clock. `None` otherwise.
    pub fn use_key<T>(&mut self, file: &KeyFile, f: impl FnOnce(&UnlockedKey) -> T) -> Option<T> {
        self.expire_if_idle();
        let now = self.clock.now();
        let h = self.held.as_mut().filter(|h| {
            h.key.reviewer() == file.reviewer && h.key.key_id() == file.key && h.key.public_b64() == file.public
        })?;
        h.last_used = now;
        Some(f(&h.key))
    }
}

fn write(session: &SharedKeySession) -> std::sync::RwLockWriteGuard<'_, KeySession> {
    session.write().unwrap_or_else(|p| p.into_inner())
}

/// Make sure `session` holds `file`'s key: nothing to do when it already
/// does; else unlock with `passphrase` (argon2, **outside** the lock) and
/// hold the result. An empty `passphrase` with nothing held is
/// [`KeystoreError::Locked`]; a wrong one is the unlock's own error, and
/// the session is unchanged.
pub fn ensure_unlocked(session: &SharedKeySession, file: &KeyFile, passphrase: &str) -> Result<(), KeystoreError> {
    if write(session).use_key(file, |_| ()).is_some() {
        return Ok(());
    }
    if passphrase.is_empty() {
        return Err(KeystoreError::Locked);
    }
    let key = file.unlock(passphrase)?;
    write(session).hold(key);
    Ok(())
}

/// Sign with `file`'s key through the session: [`ensure_unlocked`] with
/// `passphrase` (empty: the held key, or [`KeystoreError::Locked`]), then
/// `f` with the held key under the lock. Every signing action of the
/// desktop app (stamps, re-confirms, re-anchors, organisation records,
/// attestations) goes through here.
pub fn with_session_key<T>(
    session: &SharedKeySession,
    file: &KeyFile,
    passphrase: &str,
    f: impl FnOnce(&UnlockedKey) -> T,
) -> Result<T, KeystoreError> {
    ensure_unlocked(session, file, passphrase)?;
    // Locked between the two (Lock now pressed mid-job): refuse, unsigned.
    write(session).use_key(file, f).ok_or(KeystoreError::Locked)
}
