//! **The keystore**: one ed25519 key per reviewer, generated inside kovan,
//! its private half encrypted at rest (GitHub #762; #739 "Stamp signing
//! inside kovan", 2026-10-07). **Native only**: this module does not exist
//! on wasm32, and `kovan-cli` never calls it (the CLI has no signing
//! command).
//!
//! ```text
//!   passphrase ──argon2id(salt, m, t, p)──> 32-byte key ──┐
//!   ed25519 seed (32 bytes, OS entropy) ──AES-256-GCM(nonce, AAD = header)──> ciphertext
//!
//!   <kovan config dir>/keys/<reviewer>--<key>.kovankey      (TOML, mode 0600 on unix)
//!     format, reviewer, key, public, created   <- the header, bound in as AAD
//!     [kdf]    alg = "argon2id", m_cost, t_cost, p_cost, salt
//!     [cipher] alg = "aes-256-gcm", nonce, ciphertext
//! ```
//!
//! The config dir is the one kovan already uses
//! (`directories::ProjectDirs::from("org", "OUTRAM PARK", "kovan")`, as in
//! `kovan::app::setup` and `kovan::corpus_repos`).
//!
//! **The passphrase and the unlocked key live in memory only**:
//! [`KeyFile::unlock`] returns an [`UnlockedKey`] the desktop app holds while
//! it is open. Nothing here writes either to disk or to an OS keyring; the
//! signing key is zeroised when the [`UnlockedKey`] is dropped
//! (`ed25519-dalek`'s `ZeroizeOnDrop`), and the derived AES key and the
//! decrypted seed are zeroised after use.
//!
//! An [`UnlockedKey`] signs stamps ([`UnlockedKey::sign_review`],
//! [`UnlockedKey::sign_architecture`]) and registry statements
//! ([`UnlockedKey::endorse`], [`UnlockedKey::admit`],
//! [`UnlockedKey::revoke`], [`UnlockedKey::unretire`]); what each one
//! signs is in [`super`].

use std::path::{Path, PathBuf};

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use ed25519_dalek::{Signer as _, SigningKey};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use super::super::review_md::{ArchitectureEntry, ReviewEntry};
use super::super::root::{
    KeyEvent, KeyEventKind, KeySignature, KeySigner, Revocation, Reviewer, ReviewerKey,
};
use super::super::signed_at::{now_utc, parse_rfc3339};
use super::super::types::{reviewer_id_kind, FieldError};
use super::registry::{check_append_date, open_retirement, LifecycleError};
use super::{
    architecture_signed_bytes, decode_fixed, encode_b64, is_date, key_event_bytes, line,
    revocation_bytes, signed_bytes, Signature, ALG,
};

/// `format` of a key file.
pub const KEY_FILE_FORMAT: &str = "kovan-ed25519-key-v1";
/// The key file extension.
pub const KEY_FILE_EXT: &str = "kovankey";
/// argon2id memory cost (KiB) for new keys: argon2's default, 19 MiB
/// (OWASP's minimum recommendation for argon2id, t = 2, p = 1).
pub const ARGON2_M_COST: u32 = Params::DEFAULT_M_COST;
/// argon2id passes for new keys.
pub const ARGON2_T_COST: u32 = Params::DEFAULT_T_COST;
/// argon2id lanes for new keys.
pub const ARGON2_P_COST: u32 = Params::DEFAULT_P_COST;
/// The largest memory cost a key file may ask for (1 GiB): a damaged or
/// hostile file cannot make unlocking exhaust memory.
pub const ARGON2_M_COST_MAX: u32 = 1 << 20;

/// Why the keystore cannot do what was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeystoreError {
    /// A file or folder could not be read or written.
    Io { path: PathBuf, message: String },
    /// The key file is not TOML of the expected shape.
    Parse { path: Option<PathBuf>, message: String },
    /// `format` is not [`KEY_FILE_FORMAT`], or a `[kdf]`/`[cipher]` `alg`
    /// is not argon2id / aes-256-gcm.
    Unsupported(String),
    /// A base64 field of the file does not decode to the right length.
    BadField(String),
    /// The argon2 parameters are out of range (or above [`ARGON2_M_COST_MAX`]).
    Kdf(String),
    /// The reviewer id is not acceptable.
    Field(FieldError),
    /// A key id must be 1 to 64 of `[A-Za-z0-9_-]` (it is part of a file name).
    BadKeyId(String),
    /// A date is not `YYYY-MM-DD`.
    BadDate(String),
    /// The passphrase is empty.
    EmptyPassphrase,
    /// Decryption failed: a wrong passphrase, or the file was altered (the
    /// two cannot be told apart, by design of the AEAD).
    WrongPassphrase,
    /// The decrypted key does not match the file's `public` key.
    KeyMismatch,
    /// The OS entropy source failed.
    Random(String),
    /// The platform reports no home directory for kovan's config folder.
    NoConfigDir,
    /// A key file for this reviewer and key id already exists; nothing is
    /// overwritten.
    AlreadyExists(PathBuf),
    /// The file holds another reviewer's or key's key than the one asked for.
    WrongFile { path: PathBuf },
}

impl std::fmt::Display for KeystoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, message } => write!(f, "{}: {message}", path.display()),
            Self::Parse { path: Some(p), message } => write!(f, "{}: {message}", p.display()),
            Self::Parse { path: None, message } => write!(f, "key file: {message}"),
            Self::Unsupported(s) => write!(f, "unsupported key file: {s}"),
            Self::BadField(s) => write!(f, "key file field {s} is malformed"),
            Self::Kdf(s) => write!(f, "argon2 parameters: {s}"),
            Self::Field(e) => write!(f, "{e}"),
            Self::BadKeyId(s) => write!(f, "key id {s:?} must be 1-64 of A-Z a-z 0-9 _ -"),
            Self::BadDate(s) => write!(f, "{s:?} is not YYYY-MM-DD"),
            Self::EmptyPassphrase => write!(f, "the passphrase is empty"),
            Self::WrongPassphrase => write!(f, "wrong passphrase (or the key file was altered)"),
            Self::KeyMismatch => write!(f, "the decrypted key does not match its public key"),
            Self::Random(s) => write!(f, "OS entropy: {s}"),
            Self::NoConfigDir => write!(f, "no config folder for kovan on this platform"),
            Self::AlreadyExists(p) => write!(f, "{} already exists", p.display()),
            Self::WrongFile { path } => write!(f, "{} holds a different key", path.display()),
        }
    }
}

impl std::error::Error for KeystoreError {}

/// Why an [`UnlockedKey`] refuses to sign.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignError {
    /// The statement names reviewer `expected`, but this key is `signer`'s.
    WrongReviewer { expected: String, signer: String },
    /// A field the statement needs is absent (`admitted`, a first key, …).
    Missing(&'static str),
    /// [`UnlockedKey::unretire`] was given another key's entry.
    WrongKey,
    /// [`UnlockedKey::unretire`] on a key that is not retired.
    NotRetired,
    /// A date is not `YYYY-MM-DD`.
    BadDate(String),
    /// The event cannot be appended (a bad or backwards date).
    Lifecycle(LifecycleError),
    /// A `signed_at` given to [`UnlockedKey::sign_review_at`] is not RFC
    /// 3339 to the second with an offset ([`parse_rfc3339`]).
    BadSignedAt(String),
}

impl std::fmt::Display for SignError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongReviewer { expected, signer } => {
                write!(f, "this is {signer}'s key; the entry is by {expected}")
            }
            Self::Missing(what) => write!(f, "{what} is missing"),
            Self::WrongKey => write!(f, "that is not this key's registry entry"),
            Self::NotRetired => write!(f, "the key is not retired (with a date)"),
            Self::BadDate(d) => write!(f, "bad date {d:?}"),
            Self::Lifecycle(e) => write!(f, "{e}"),
            Self::BadSignedAt(t) => write!(f, "signed_at {t:?} is not RFC 3339 to the second"),
        }
    }
}

impl std::error::Error for SignError {}

fn check_signed_at(signed_at: &str) -> Result<(), SignError> {
    match parse_rfc3339(signed_at) {
        Some(_) => Ok(()),
        None => Err(SignError::BadSignedAt(signed_at.to_string())),
    }
}

/// `[kdf]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfParams {
    pub alg: String,
    pub m_cost: u32,
    pub t_cost: u32,
    pub p_cost: u32,
    /// Base64, 16 bytes.
    pub salt: String,
}

/// `[cipher]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CipherBlob {
    pub alg: String,
    /// Base64, 12 bytes.
    pub nonce: String,
    /// Base64: the 32-byte ed25519 seed plus the 16-byte GCM tag.
    pub ciphertext: String,
}

/// One encrypted key file. Holds nothing secret in the clear.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyFile {
    pub format: String,
    pub reviewer: String,
    pub key: String,
    /// Base64 ed25519 public key (what goes in `[[reviewer.key]] public`).
    pub public: String,
    pub created: String,
    pub kdf: KdfParams,
    pub cipher: CipherBlob,
}

fn check_key_id(id: &str) -> Result<(), KeystoreError> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if ok {
        Ok(())
    } else {
        Err(KeystoreError::BadKeyId(id.to_string()))
    }
}

fn random<const N: usize>() -> Result<Zeroizing<[u8; N]>, KeystoreError> {
    let mut b = Zeroizing::new([0u8; N]);
    getrandom::fill(b.as_mut()).map_err(|e| KeystoreError::Random(e.to_string()))?;
    Ok(b)
}

fn derive(passphrase: &str, kdf: &KdfParams) -> Result<Zeroizing<[u8; 32]>, KeystoreError> {
    if passphrase.is_empty() {
        return Err(KeystoreError::EmptyPassphrase);
    }
    if kdf.alg != "argon2id" {
        return Err(KeystoreError::Unsupported(format!("kdf {}", kdf.alg)));
    }
    if kdf.m_cost > ARGON2_M_COST_MAX {
        return Err(KeystoreError::Kdf(format!("m_cost {} above {ARGON2_M_COST_MAX}", kdf.m_cost)));
    }
    let params = Params::new(kdf.m_cost, kdf.t_cost, kdf.p_cost, Some(32))
        .map_err(|e| KeystoreError::Kdf(e.to_string()))?;
    let salt = decode_fixed::<16>("kdf.salt", &kdf.salt)
        .map_err(|_| KeystoreError::BadField("kdf.salt".into()))?;
    let mut out = Zeroizing::new([0u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(passphrase.as_bytes(), &salt, out.as_mut())
        .map_err(|e| KeystoreError::Kdf(e.to_string()))?;
    Ok(out)
}

/// Generate a new key for `reviewer`, encrypted under `passphrase`. Returns
/// the file to save ([`Keystore::save`]) and the unlocked key. The new
/// key's registry entry is [`KeyFile::reviewer_key`]; it still needs an
/// admission (first key) or an endorsement (later keys).
pub fn generate(
    reviewer: &str,
    key_id: &str,
    created: &str,
    passphrase: &str,
) -> Result<(KeyFile, UnlockedKey), KeystoreError> {
    reviewer_id_kind(reviewer).map_err(KeystoreError::Field)?;
    check_key_id(key_id)?;
    if !is_date(created) {
        return Err(KeystoreError::BadDate(created.to_string()));
    }
    if passphrase.is_empty() {
        return Err(KeystoreError::EmptyPassphrase);
    }
    let seed = random::<32>()?;
    let signing = SigningKey::from_bytes(&seed);
    let public = encode_b64(signing.verifying_key().as_bytes());
    let file = seal(reviewer, key_id, created, &public, &seed, passphrase)?;
    let unlocked = UnlockedKey { reviewer: reviewer.into(), key: key_id.into(), signing };
    Ok((file, unlocked))
}

/// Encrypt `seed` under `passphrase` with a fresh salt and nonce, binding
/// the header (`public` as given) in as associated data. `generate` passes
/// the seed's own public key; a test passes another to reach
/// [`KeystoreError::KeyMismatch`].
pub(super) fn seal(
    reviewer: &str,
    key_id: &str,
    created: &str,
    public: &str,
    seed: &[u8; 32],
    passphrase: &str,
) -> Result<KeyFile, KeystoreError> {
    let salt = random::<16>()?;
    let nonce = random::<12>()?;
    let mut file = KeyFile {
        format: KEY_FILE_FORMAT.into(),
        reviewer: reviewer.into(),
        key: key_id.into(),
        public: public.into(),
        created: created.into(),
        kdf: KdfParams {
            alg: "argon2id".into(),
            m_cost: ARGON2_M_COST,
            t_cost: ARGON2_T_COST,
            p_cost: ARGON2_P_COST,
            salt: encode_b64(salt.as_ref()),
        },
        cipher: CipherBlob {
            alg: "aes-256-gcm".into(),
            nonce: encode_b64(nonce.as_ref()),
            ciphertext: String::new(),
        },
    };
    let aes_key = derive(passphrase, &file.kdf)?;
    let cipher = Aes256Gcm::new_from_slice(aes_key.as_ref())
        .map_err(|e| KeystoreError::Kdf(e.to_string()))?;
    let aad = file.aad();
    let ct = cipher
        .encrypt(Nonce::from_slice(nonce.as_ref()), Payload { msg: seed.as_ref(), aad: &aad })
        .map_err(|_| KeystoreError::Kdf("AES-GCM encryption failed".into()))?;
    file.cipher.ciphertext = encode_b64(&ct);
    Ok(file)
}

impl KeyFile {
    /// The header bound into the ciphertext as AES-GCM associated data, so
    /// editing the reviewer, key id, public key or date breaks decryption.
    fn aad(&self) -> Vec<u8> {
        let mut s = String::from("kovan-key-file-aad-v1\n");
        line(&mut s, "format", &[&self.format]);
        line(&mut s, "reviewer", &[&self.reviewer]);
        line(&mut s, "key", &[&self.key]);
        line(&mut s, "public", &[&self.public]);
        line(&mut s, "created", &[&self.created]);
        s.into_bytes()
    }

    /// Decrypt with `passphrase`. The result lives in memory only.
    pub fn unlock(&self, passphrase: &str) -> Result<UnlockedKey, KeystoreError> {
        if self.format != KEY_FILE_FORMAT {
            return Err(KeystoreError::Unsupported(format!("format {}", self.format)));
        }
        if self.cipher.alg != "aes-256-gcm" {
            return Err(KeystoreError::Unsupported(format!("cipher {}", self.cipher.alg)));
        }
        let nonce = decode_fixed::<12>("cipher.nonce", &self.cipher.nonce)
            .map_err(|_| KeystoreError::BadField("cipher.nonce".into()))?;
        let ct = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &self.cipher.ciphertext)
            .map_err(|_| KeystoreError::BadField("cipher.ciphertext".into()))?;
        let aes_key = derive(passphrase, &self.kdf)?;
        let cipher = Aes256Gcm::new_from_slice(aes_key.as_ref())
            .map_err(|e| KeystoreError::Kdf(e.to_string()))?;
        let aad = self.aad();
        let seed = Zeroizing::new(
            cipher
                .decrypt(Nonce::from_slice(&nonce), Payload { msg: &ct, aad: &aad })
                .map_err(|_| KeystoreError::WrongPassphrase)?,
        );
        let seed: Zeroizing<[u8; 32]> = Zeroizing::new(
            seed.as_slice().try_into().map_err(|_| KeystoreError::BadField("cipher.ciphertext".into()))?,
        );
        let signing = SigningKey::from_bytes(&seed);
        if encode_b64(signing.verifying_key().as_bytes()) != self.public {
            return Err(KeystoreError::KeyMismatch);
        }
        Ok(UnlockedKey { reviewer: self.reviewer.clone(), key: self.key.clone(), signing })
    }

    /// The `[[reviewer.key]]` entry for this key (no endorsement yet).
    pub fn reviewer_key(&self) -> ReviewerKey {
        ReviewerKey {
            id: self.key.clone(),
            alg: ALG.into(),
            public: self.public.clone(),
            created: self.created.clone(),
            endorsed_by: None,
            reset: false,
            retired: false,
            retired_on: None,
            unretired: None,
            history: vec![KeyEvent::unsigned(KeyEventKind::Created, &self.created)],
        }
    }

    /// The file's TOML text.
    pub fn to_toml(&self) -> Result<String, KeystoreError> {
        toml::to_string_pretty(self).map_err(|e| KeystoreError::Parse { path: None, message: e.to_string() })
    }

    /// Read a key file's TOML text.
    pub fn from_toml(text: &str) -> Result<KeyFile, KeystoreError> {
        toml::from_str(text).map_err(|e| KeystoreError::Parse { path: None, message: e.to_string() })
    }
}

/// An unlocked key, in memory only. Not `Clone`; `Debug` never prints the
/// secret; zeroised on drop.
pub struct UnlockedKey {
    reviewer: String,
    key: String,
    signing: SigningKey,
}

impl std::fmt::Debug for UnlockedKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnlockedKey")
            .field("reviewer", &self.reviewer)
            .field("key", &self.key)
            .field("signing", &"<secret>")
            .finish()
    }
}

impl UnlockedKey {
    /// The reviewer id this key belongs to.
    pub fn reviewer(&self) -> &str {
        &self.reviewer
    }

    /// The key id.
    pub fn key_id(&self) -> &str {
        &self.key
    }

    /// The base64 public key.
    pub fn public_b64(&self) -> String {
        encode_b64(self.signing.verifying_key().as_bytes())
    }

    fn sign(&self, msg: &[u8]) -> String {
        encode_b64(&self.signing.sign(msg).to_bytes())
    }

    fn own(&self, by: &str) -> Result<(), SignError> {
        if by == self.reviewer {
            Ok(())
        } else {
            Err(SignError::WrongReviewer { expected: by.into(), signer: self.reviewer.clone() })
        }
    }

    fn key_signature(&self, msg: &[u8]) -> KeySignature {
        KeySignature { key: self.key.clone(), signature: self.sign(msg) }
    }

    /// Sign a review this reviewer wrote (`[review] by` must be this key's
    /// reviewer); sets `[review] signed_at` from the clock, in UTC
    /// ([`now_utc`]; GitHub #783), and `[review.signature]` over the v2
    /// signed bytes.
    pub fn sign_review(&self, r: &mut ReviewEntry) -> Result<(), SignError> {
        self.sign_review_at(r, &now_utc())
    }

    /// [`Self::sign_review`] at a given `signed_at` (RFC 3339 to the second,
    /// with offset): for a caller that knows the local offset, and for
    /// tests. Refuses a `signed_at` that does not parse; nothing is changed
    /// on any refusal.
    pub fn sign_review_at(&self, r: &mut ReviewEntry, signed_at: &str) -> Result<(), SignError> {
        self.own(&r.review.by)?;
        check_signed_at(signed_at)?;
        r.review.signed_at = Some(signed_at.to_string());
        let value = self.sign(&signed_bytes(r));
        r.review.signature = Some(Signature { key: self.key.clone(), alg: ALG.into(), value });
        Ok(())
    }

    /// Sign an architecture node this reviewer recorded; sets `signed_at`
    /// from the clock as [`Self::sign_review`] does.
    pub fn sign_architecture(&self, a: &mut ArchitectureEntry) -> Result<(), SignError> {
        self.sign_architecture_at(a, &now_utc())
    }

    /// [`Self::sign_architecture`] at a given `signed_at`.
    pub fn sign_architecture_at(&self, a: &mut ArchitectureEntry, signed_at: &str) -> Result<(), SignError> {
        self.own(&a.architecture.by)?;
        check_signed_at(signed_at)?;
        a.architecture.signed_at = Some(signed_at.to_string());
        let value = self.sign(&architecture_signed_bytes(a));
        a.architecture.signature = Some(Signature { key: self.key.clone(), alg: ALG.into(), value });
        Ok(())
    }

    /// Append a signed event to `owner`'s key `k` (signer = this key).
    fn sign_event(
        &self,
        owner: &str,
        k: &mut ReviewerKey,
        event: KeyEventKind,
        date: &str,
        retired_on: &str,
        admission: Option<&Reviewer>,
    ) -> Result<(), SignError> {
        check_append_date(k, date).map_err(SignError::Lifecycle)?;
        let mut ev = KeyEvent {
            event,
            date: date.into(),
            signer: Some(KeySigner { reviewer: Some(self.reviewer.clone()), key: self.key.clone() }),
            signature: None,
            legacy: false,
        };
        ev.signature = Some(self.sign(&key_event_bytes(owner, k, &ev, retired_on, admission)));
        k.history.push(ev);
        Ok(())
    }

    /// Endorse `owner`'s new key `k` from `date` with one of the owner's own
    /// keys: appends an `endorsed` event.
    pub fn endorse(&self, owner: &str, k: &mut ReviewerKey, date: &str) -> Result<(), SignError> {
        self.sign_event(owner, k, KeyEventKind::Endorsed, date, "", None)
    }

    /// Vouch for `owner`'s reset key `k` (the old passphrase is lost) as a
    /// maintainer: appends a `reset` event, shown permanently.
    pub fn endorse_reset(&self, owner: &str, k: &mut ReviewerKey, date: &str) -> Result<(), SignError> {
        self.sign_event(owner, k, KeyEventKind::Reset, date, "", None)
    }

    /// Admit reviewer `r` as a maintainer: appends an `admitted` event, dated
    /// `r.admitted`, to its first key. `admitted` and a first key must
    /// already be set: the admission covers them, its role and its scope.
    pub fn admit(&self, r: &mut Reviewer) -> Result<(), SignError> {
        let d = r.admitted.clone().ok_or(SignError::Missing("admitted"))?;
        if !is_date(&d) {
            return Err(SignError::BadDate(d));
        }
        if r.keys.is_empty() {
            return Err(SignError::Missing("a first key"));
        }
        let snapshot = r.clone();
        let owner = r.id.clone();
        self.sign_event(&owner, &mut r.keys[0], KeyEventKind::Admitted, &d, "", Some(&snapshot))
    }

    /// Sign a revocation of `reviewer` (`rev.by` must be this key's
    /// reviewer); sets `rev.signature`.
    pub fn revoke(&self, reviewer: &str, rev: &mut Revocation) -> Result<(), SignError> {
        self.own(&rev.by)?;
        rev.signature = Some(self.key_signature(&revocation_bytes(reviewer, rev)));
        Ok(())
    }

    /// Revoke one key of `owner` from `date` (`compromised`: it leaked, and
    /// is void from `date`): appends a signed `revoked` or `compromised`
    /// event. Signed by a maintainer, the owner, or the key itself.
    pub fn revoke_key(
        &self,
        owner: &str,
        k: &mut ReviewerKey,
        date: &str,
        compromised: bool,
    ) -> Result<(), SignError> {
        let kind = if compromised { KeyEventKind::Compromised } else { KeyEventKind::Revoked };
        self.sign_event(owner, k, kind, date, "", None)
    }

    /// Un-retire **this** key from `date`: the possession proof (#739),
    /// possible only with the key unlocked from its encrypted file. Appends
    /// an `unretired` event signed by the key itself; `k` must be this key
    /// and retired now.
    pub fn unretire(&self, k: &mut ReviewerKey, date: &str) -> Result<(), SignError> {
        if k.id != self.key || k.public != self.public_b64() {
            return Err(SignError::WrongKey);
        }
        let retired_on = open_retirement(k).ok_or(SignError::NotRetired)?;
        let owner = self.reviewer.clone();
        self.sign_event(&owner, k, KeyEventKind::Unretired, date, &retired_on, None)
    }
}

/// The folder holding key files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keystore {
    dir: PathBuf,
}

impl Keystore {
    /// A keystore in `dir` (created on the first save).
    pub fn at(dir: impl Into<PathBuf>) -> Keystore {
        Keystore { dir: dir.into() }
    }

    /// `<kovan config dir>/keys`. Creates nothing.
    pub fn default_location() -> Result<Keystore, KeystoreError> {
        directories::ProjectDirs::from("org", "OUTRAM PARK", "kovan")
            .map(|d| Keystore::at(d.config_dir().join("keys")))
            .ok_or(KeystoreError::NoConfigDir)
    }

    /// The folder.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Where the key file of `reviewer`'s key `key` lives.
    pub fn path_for(&self, reviewer: &str, key: &str) -> PathBuf {
        let safe: String = reviewer
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' })
            .collect();
        self.dir.join(format!("{safe}--{key}.{KEY_FILE_EXT}"))
    }

    /// Write a new key file (owner-only permissions on unix). Never
    /// overwrites: an existing file is [`KeystoreError::AlreadyExists`].
    pub fn save(&self, file: &KeyFile) -> Result<PathBuf, KeystoreError> {
        use std::io::Write as _;
        check_key_id(&file.key)?;
        let io = |path: &Path, e: std::io::Error| KeystoreError::Io {
            path: path.to_path_buf(),
            message: e.to_string(),
        };
        std::fs::create_dir_all(&self.dir).map_err(|e| io(&self.dir, e))?;
        let path = self.path_for(&file.reviewer, &file.key);
        let text = file.to_toml()?;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            opts.mode(0o600);
        }
        let mut f = opts.open(&path).map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => KeystoreError::AlreadyExists(path.clone()),
            _ => io(&path, e),
        })?;
        f.write_all(text.as_bytes()).map_err(|e| io(&path, e))?;
        f.sync_all().map_err(|e| io(&path, e))?;
        Ok(path)
    }

    /// Read `reviewer`'s key `key` (still encrypted).
    pub fn load(&self, reviewer: &str, key: &str) -> Result<KeyFile, KeystoreError> {
        let path = self.path_for(reviewer, key);
        let file = read_key_file(&path)?;
        if file.reviewer != reviewer || file.key != key {
            return Err(KeystoreError::WrongFile { path });
        }
        Ok(file)
    }

    /// Every key file in the folder, sorted by file name (none when the
    /// folder does not exist yet). A file that does not parse is an error,
    /// never skipped.
    pub fn list(&self) -> Result<Vec<KeyFile>, KeystoreError> {
        let entries = match std::fs::read_dir(&self.dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(KeystoreError::Io { path: self.dir.clone(), message: e.to_string() }),
        };
        let mut paths = Vec::new();
        for e in entries {
            let p = e.map_err(|e| KeystoreError::Io { path: self.dir.clone(), message: e.to_string() })?.path();
            if p.extension().and_then(|x| x.to_str()) == Some(KEY_FILE_EXT) {
                paths.push(p);
            }
        }
        paths.sort();
        paths.iter().map(|p| read_key_file(p)).collect()
    }
}

fn read_key_file(path: &Path) -> Result<KeyFile, KeystoreError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| KeystoreError::Io { path: path.to_path_buf(), message: e.to_string() })?;
    KeyFile::from_toml(&text).map_err(|e| match e {
        KeystoreError::Parse { message, .. } => KeystoreError::Parse { path: Some(path.to_path_buf()), message },
        other => other,
    })
}
