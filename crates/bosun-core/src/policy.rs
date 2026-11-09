//! Güvenlik politikası: hangi yolların asla dokunulmaz olduğu ve bir yolun
//! izin verilen bir kök altında olup olmadığının doğrulanması.

use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PolicyError {
    #[error("path is not absolute: {0}")]
    NotAbsolute(PathBuf),
    #[error("path not found: {0}")]
    Missing(PathBuf),
    #[error("symbolic links are not processed: {0}")]
    Symlink(PathBuf),
    #[error("protected path: {0}")]
    Protected(PathBuf),
    #[error("outside the allowed root: {0}")]
    OutsideRoot(PathBuf),
    #[error("the root itself cannot be removed: {0}")]
    IsRoot(PathBuf),
}

/// Silinmesi hiçbir koşulda kabul edilmeyen sistem yolları.
const SYSTEM_PROTECTED: &[&str] = &[
    "/", "/bin", "/boot", "/dev", "/etc", "/home", "/lib", "/lib32", "/lib64", "/libx32",
    "/opt", "/proc", "/root", "/run", "/sbin", "/snap", "/srv", "/sys", "/usr", "/var",
    "/var/lib", "/var/log", "/var/cache", "/tmp",
];

/// Ev dizini altında asla silinmeyecek klasörler (Türkçe ve İngilizce adlarıyla).
const HOME_PROTECTED: &[&str] = &[
    "Documents", "Belgeler", "Pictures", "Resimler", "Desktop", "Masaüstü", "Music", "Müzik",
    "Videos", "Videolar", "Templates", "Şablonlar", "Public", "Genel", "Sync", "Vaults",
    ".ssh", ".gnupg", ".password-store", ".pki", ".config", ".local", ".local/share",
    ".local/share/keyrings", ".mozilla", ".cargo", ".rustup", ".npm", ".cache", ".git",
];

#[derive(Debug, Clone)]
pub struct Policy {
    home: PathBuf,
    protected: Vec<PathBuf>,
}

impl Policy {
    pub fn new(home: impl Into<PathBuf>) -> Self {
        let home: PathBuf = home.into();
        let home = home.canonicalize().unwrap_or(home);
        let mut protected: Vec<PathBuf> = SYSTEM_PROTECTED.iter().map(PathBuf::from).collect();
        protected.push(home.clone());
        protected.extend(HOME_PROTECTED.iter().map(|p| home.join(p)));
        Self { home, protected }
    }

    /// Geçerli kullanıcının ev dizini ile politika.
    pub fn for_current_user() -> Self {
        Self::new(dirs::home_dir().unwrap_or_else(|| PathBuf::from("/nonexistent")))
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    /// Yol korumalı bir yolun kendisi ya da onun bir *atası* ise korumalıdır.
    /// (Örneğin `~` silinirse `~/Belgeler` de gider; bu yüzden ata da yasaktır.)
    pub fn is_protected(&self, path: &Path) -> bool {
        self.protected.iter().any(|p| p == path || p.starts_with(path))
    }

    /// Bir yolu silinmeden hemen önce doğrular ve kanonik halini döndürür.
    ///
    /// Kurallar: mutlak olmalı, var olmalı, kendisi sembolik bağ olmamalı,
    /// kanonik hali `allowed_root` altında (ama kökün kendisi değil) olmalı ve korumalı olmamalı.
    pub fn validate(&self, path: &Path, allowed_root: &Path) -> Result<PathBuf, PolicyError> {
        if !path.is_absolute() {
            return Err(PolicyError::NotAbsolute(path.to_path_buf()));
        }
        let meta = std::fs::symlink_metadata(path)
            .map_err(|_| PolicyError::Missing(path.to_path_buf()))?;
        if meta.file_type().is_symlink() {
            // Bağın kendisi silinebilir ama hedefi asla; basitlik için hiç dokunmuyoruz.
            return Err(PolicyError::Symlink(path.to_path_buf()));
        }
        let canonical = path
            .canonicalize()
            .map_err(|_| PolicyError::Missing(path.to_path_buf()))?;
        let root = allowed_root
            .canonicalize()
            .map_err(|_| PolicyError::Missing(allowed_root.to_path_buf()))?;
        if canonical == root {
            return Err(PolicyError::IsRoot(canonical));
        }
        if !canonical.starts_with(&root) {
            return Err(PolicyError::OutsideRoot(canonical));
        }
        if self.is_protected(&canonical) {
            return Err(PolicyError::Protected(canonical));
        }
        Ok(canonical)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup() -> (tempfile::TempDir, Policy) {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        fs::create_dir_all(home.join(".cache/app")).unwrap();
        fs::create_dir_all(home.join("Belgeler")).unwrap();
        fs::write(home.join(".cache/app/a.bin"), b"x").unwrap();
        let policy = Policy::new(&home);
        (dir, policy)
    }

    #[test]
    fn system_paths_are_protected() {
        let (_d, p) = setup();
        for path in ["/", "/usr", "/etc", "/boot", "/var/lib"] {
            assert!(p.is_protected(Path::new(path)), "{path}");
        }
    }

    #[test]
    fn home_and_its_ancestors_are_protected() {
        let (_d, p) = setup();
        assert!(p.is_protected(p.home()));
        assert!(p.is_protected(p.home().parent().unwrap()));
        assert!(p.is_protected(&p.home().join("Belgeler")));
    }

    #[test]
    fn file_inside_cache_is_allowed() {
        let (_d, p) = setup();
        let root = p.home().join(".cache");
        let ok = p.validate(&root.join("app/a.bin"), &root).unwrap();
        assert!(ok.ends_with("app/a.bin"));
        assert!(p.validate(&root.join("app"), &root).is_ok());
    }

    #[test]
    fn root_itself_is_rejected() {
        let (_d, p) = setup();
        let root = p.home().join(".cache");
        assert!(matches!(p.validate(&root, &root), Err(PolicyError::IsRoot(_))));
    }

    #[test]
    fn traversal_outside_root_is_rejected() {
        let (_d, p) = setup();
        let root = p.home().join(".cache");
        let sneaky = root.join("../Belgeler");
        assert!(matches!(p.validate(&sneaky, &root), Err(PolicyError::OutsideRoot(_))));
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_rejected() {
        let (_d, p) = setup();
        let root = p.home().join(".cache");
        std::os::unix::fs::symlink(p.home().join("Belgeler"), root.join("link")).unwrap();
        assert!(matches!(p.validate(&root.join("link"), &root), Err(PolicyError::Symlink(_))));
    }

    #[test]
    fn relative_and_missing_are_rejected() {
        let (_d, p) = setup();
        let root = p.home().join(".cache");
        assert!(matches!(p.validate(Path::new("a.bin"), &root), Err(PolicyError::NotAbsolute(_))));
        assert!(matches!(p.validate(&root.join("yok"), &root), Err(PolicyError::Missing(_))));
    }
}
