//! NASA API key storage (PLAN §5.4, R14).
//! Windows port: Credential Manager (DPAPI-backed). This Linux build uses the
//! documented fallback: a 0600 file under $XDG_CONFIG_HOME/orrery/ (never in
//! catalog.json, never logged). STORE = NO keeps the key in-session only.

use std::path::PathBuf;

fn key_path() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/tmp".into())).join(".config"));
    base.join("orrery").join("nasa_key")
}

pub fn load() -> Option<String> {
    std::fs::read_to_string(key_path()).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

pub fn store(key: &str) -> std::io::Result<()> {
    let p = key_path();
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&p, key)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

pub fn clear() {
    let _ = std::fs::remove_file(key_path());
}

/// Mask for the UI: `•••••••` with show/hide toggle handled by the panel.
pub fn mask(key: &str) -> String {
    if key.is_empty() { "NONE".into() } else { "•".repeat(key.len().min(12)) }
}
