//! Persistent host-side store of minted LiteLLM virtual-key tokens (`key_alias -> token`).
//!
//! Tokens have the shape `sk-llmsc-<sandbox>-<agent>-<random>`: a deterministic, owner-identifying
//! **prefix** (legible in LiteLLM, Phoenix traces, and logs) plus a **random secret suffix** that
//! is the unit of rotation. The suffix can't be recomputed, so once minted a token is persisted
//! here (mode 0600) to be injected into agents and rotated later.
//!
//! These are *virtual* keys — budget/model-scoped, reachable only via the localhost proxy — not
//! provider credentials. Real provider keys live only inside the `svc-litellm` container and never
//! touch this store or `llmsc.toml`.

use crate::error::{Error, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Build a virtual-key token from an owner-identifying prefix and a secret suffix. Pure (the
/// suffix is supplied) so token construction is unit-testable; production callers pass
/// [`random_suffix`].
pub fn key_token(sandbox: &str, agent: &str, suffix: &str) -> String {
    format!("sk-llmsc-{sandbox}-{agent}-{suffix}")
}

/// A fresh 128-bit random secret as 32 lowercase hex chars, from the OS CSPRNG (`getrandom`).
pub fn random_suffix() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("OS RNG unavailable");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Host-side persistent map of `key_alias -> token` for minted virtual keys.
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct KeyStore {
    #[serde(default)]
    tokens: BTreeMap<String, String>,
}

impl KeyStore {
    /// Load the store from `path`; a missing file is an empty store (not an error).
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(s) => serde_json::from_str(&s)
                .map_err(|e| Error::Config(format!("parsing key store {}: {e}", path.display()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(Error::Config(format!(
                "reading key store {}: {e}",
                path.display()
            ))),
        }
    }

    /// Persist the store to `path` (parent dirs created; file mode 0600 on unix).
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::Config(format!("creating {}: {e}", parent.display())))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| Error::Config(format!("serializing key store: {e}")))?;
        std::fs::write(path, json)
            .map_err(|e| Error::Config(format!("writing key store {}: {e}", path.display())))?;
        set_owner_only(path)
    }

    /// The token for an alias, if one has been minted.
    pub fn get(&self, alias: &str) -> Option<&str> {
        self.tokens.get(alias).map(String::as_str)
    }

    /// Insert or replace an alias's token (sync mints, rotate replaces).
    pub fn upsert(&mut self, alias: impl Into<String>, token: impl Into<String>) {
        self.tokens.insert(alias.into(), token.into());
    }

    /// Forget an alias (revoke/rotate). Returns the old token if present.
    pub fn remove(&mut self, alias: &str) -> Option<String> {
        self.tokens.remove(alias)
    }

    /// The full `alias -> token` map (e.g. to seed a sync with already-minted tokens).
    pub fn tokens(&self) -> &BTreeMap<String, String> {
        &self.tokens
    }

    /// How many of the given aliases have a persisted token (e.g. compiled-vs-minted in `doctor`).
    pub fn count_minted<'a>(&self, aliases: impl IntoIterator<Item = &'a str>) -> usize {
        aliases
            .into_iter()
            .filter(|a| self.tokens.contains_key(*a))
            .count()
    }
}

#[cfg(unix)]
fn set_owner_only(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| Error::Config(format!("chmod 600 {}: {e}", path.display())))
}

#[cfg(not(unix))]
fn set_owner_only(_path: &Path) -> Result<()> {
    Ok(())
}

/// Default store path: `$XDG_STATE_HOME/llmsc/keys.json` (falling back to `~/.local/state`).
pub fn default_key_store_path() -> PathBuf {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("state")))
        .unwrap_or_else(|| PathBuf::from(".local").join("state"));
    base.join("llmsc").join("keys.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_token_has_identifiable_prefix_and_suffix() {
        assert_eq!(
            key_token("web-agent-01", "agent-claude", "deadbeef"),
            "sk-llmsc-web-agent-01-agent-claude-deadbeef"
        );
    }

    #[test]
    fn random_suffix_is_32_hex_chars_and_unpredictable() {
        let a = random_suffix();
        let b = random_suffix();
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b, "two suffixes should not collide");
    }

    #[test]
    fn store_round_trips_and_missing_file_is_empty() {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "llmsc-keystore-{}-{}.json",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_file(&path);

        // Missing file → empty store.
        assert!(KeyStore::load(&path).unwrap().get("nope").is_none());

        let mut store = KeyStore::default();
        store.upsert("llmsc-sb-agent-x", "sk-llmsc-sb-agent-x-abc123");
        store.save(&path).unwrap();

        let loaded = KeyStore::load(&path).unwrap();
        assert_eq!(
            loaded.get("llmsc-sb-agent-x"),
            Some("sk-llmsc-sb-agent-x-abc123")
        );

        // 0600 on unix.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn count_minted_counts_only_present_aliases() {
        let mut s = KeyStore::default();
        s.upsert("a", "t");
        s.upsert("b", "t");
        assert_eq!(s.count_minted(["a", "b", "c"]), 2);
        assert_eq!(s.count_minted(std::iter::empty()), 0);
    }

    #[test]
    fn remove_forgets_a_token() {
        let mut store = KeyStore::default();
        store.upsert("a", "tok");
        assert_eq!(store.remove("a"), Some("tok".to_string()));
        assert!(store.get("a").is_none());
    }

    #[test]
    fn default_path_honors_xdg_state_home() {
        // Note: process-global env; this test sets and restores XDG_STATE_HOME.
        let prev = std::env::var_os("XDG_STATE_HOME");
        std::env::set_var("XDG_STATE_HOME", "/tmp/xdgstate");
        assert_eq!(
            default_key_store_path(),
            PathBuf::from("/tmp/xdgstate/llmsc/keys.json")
        );
        match prev {
            Some(v) => std::env::set_var("XDG_STATE_HOME", v),
            None => std::env::remove_var("XDG_STATE_HOME"),
        }
    }
}
