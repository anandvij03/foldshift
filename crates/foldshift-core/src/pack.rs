use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PackError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid manifest: {0}")]
    InvalidManifest(String),
    #[error("unsafe path in pack: '{0}' ({1})")]
    UnsafePath(PathBuf, String),
    #[error(
        "incompatible target platform: pack requires os='{expected_os}', arch='{expected_arch}', host is os='{host_os}', arch='{host_arch}'"
    )]
    IncompatibleTarget {
        expected_os: String,
        expected_arch: String,
        host_os: String,
        host_arch: String,
    },
    #[error(
        "incompatible core version: pack requires core '{required}', but foldshift-core is '{current}'"
    )]
    IncompatibleCoreVersion { required: String, current: String },
    #[error("pack entrypoint not found or not executable: {0}")]
    EntrypointMissing(PathBuf),
    #[error("pack '{0}' is not installed")]
    NotInstalled(String),
}

pub type Result<T> = std::result::Result<T, PackError>;

/// Represents a single file in the pack inventory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackFileEntry {
    pub path: PathBuf,
    pub bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

/// Metadata describing an optional engine capability pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackManifest {
    pub schema_version: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub capability: String,
    pub os: String,
    pub arch: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abi: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub core_version_requirement: Option<String>,
    pub entrypoint: PathBuf,
    pub download_bytes: u64,
    pub unpacked_bytes: u64,
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub file_inventory: Vec<PackFileEntry>,
    pub license: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trusted_key_id: Option<String>,
}

impl PackManifest {
    /// Validates manifest schema, identifiers, entrypoint and inventory paths.
    pub fn validate(&self) -> Result<()> {
        if self.schema_version.trim().is_empty() {
            return Err(PackError::InvalidManifest(
                "schema_version cannot be empty".into(),
            ));
        }
        if self.id.trim().is_empty() {
            return Err(PackError::InvalidManifest("pack id cannot be empty".into()));
        }
        if self.name.trim().is_empty() {
            return Err(PackError::InvalidManifest(
                "pack name cannot be empty".into(),
            ));
        }
        if self.version.trim().is_empty() {
            return Err(PackError::InvalidManifest(
                "pack version cannot be empty".into(),
            ));
        }
        if self.capability.trim().is_empty() {
            return Err(PackError::InvalidManifest(
                "capability cannot be empty".into(),
            ));
        }
        if self.license.trim().is_empty() {
            return Err(PackError::InvalidManifest("license cannot be empty".into()));
        }

        // Validate entrypoint path
        validate_safe_path(&self.entrypoint)?;

        // Validate file inventory paths
        for entry in &self.file_inventory {
            validate_safe_path(&entry.path)?;
        }

        // If file inventory is specified, ensure entrypoint is included
        if !self.file_inventory.is_empty()
            && !self
                .file_inventory
                .iter()
                .any(|e| e.path == self.entrypoint)
        {
            return Err(PackError::InvalidManifest(format!(
                "entrypoint '{}' is not present in file_inventory",
                self.entrypoint.display()
            )));
        }

        Ok(())
    }

    /// Verifies that the manifest target matches the running host OS and architecture.
    pub fn verify_host_compatibility(&self) -> Result<()> {
        let host_os = std::env::consts::OS;
        let host_arch = std::env::consts::ARCH;

        if self.os != host_os || self.arch != host_arch {
            return Err(PackError::IncompatibleTarget {
                expected_os: self.os.clone(),
                expected_arch: self.arch.clone(),
                host_os: host_os.to_string(),
                host_arch: host_arch.to_string(),
            });
        }
        Ok(())
    }

    /// Verifies that the current foldshift-core version satisfies the manifest requirement.
    pub fn verify_core_compatibility(&self) -> Result<()> {
        let current_core = env!("CARGO_PKG_VERSION");
        if let Some(req) = &self.core_version_requirement
            && !is_core_version_compatible(req, current_core)
        {
            return Err(PackError::IncompatibleCoreVersion {
                required: req.clone(),
                current: current_core.to_string(),
            });
        }
        Ok(())
    }
}

/// Checks whether a given path is strictly relative and contains no path traversal attempts.
pub fn is_safe_relative_path(path: &Path) -> bool {
    let s = path.to_string_lossy();
    if s.is_empty() || path.is_absolute() {
        return false;
    }
    // Reject drive letters or Windows prefixes even when parsed on Unix
    if s.contains(':') {
        return false;
    }
    // Reject leading slashes
    if s.starts_with('/') || s.starts_with('\\') {
        return false;
    }
    // Split on either / or \ to check every segment across platforms
    for segment in s.split(['/', '\\']) {
        if segment == ".." || segment.contains('\0') {
            return false;
        }
    }
    // Check standard Path components
    for component in path.components() {
        match component {
            Component::Normal(c) => {
                let name = c.to_string_lossy();
                if name.contains('\0') || name == ".." {
                    return false;
                }
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return false;
            }
        }
    }
    true
}

/// Validates that a path is safe and returns PackError::UnsafePath if invalid.
pub fn validate_safe_path(path: &Path) -> Result<()> {
    if !is_safe_relative_path(path) {
        return Err(PackError::UnsafePath(
            path.to_path_buf(),
            "path must be relative and cannot contain '..', absolute roots, or drive prefixes"
                .into(),
        ));
    }
    Ok(())
}

/// Checks whether current core version satisfies the requirement specification.
pub fn is_core_version_compatible(requirement: &str, current: &str) -> bool {
    let req = requirement.trim();
    if req == "*" || req.is_empty() {
        return true;
    }
    let clean_req = req
        .strip_prefix(">=")
        .or_else(|| req.strip_prefix('^'))
        .or_else(|| req.strip_prefix('='))
        .unwrap_or(req)
        .trim();

    // Check major.minor compatibility
    let req_parts: Vec<&str> = clean_req.split('.').collect();
    let cur_parts: Vec<&str> = current.split('.').collect();

    if req_parts.is_empty() || cur_parts.is_empty() {
        return false;
    }

    // Major version must match
    if req_parts[0] != cur_parts[0] {
        return false;
    }

    // Minor version must match if specified
    if req_parts.len() > 1 && cur_parts.len() > 1 && req_parts[1] != cur_parts[1] {
        return false;
    }

    true
}

/// Information about an installed engine capability pack recorded locally.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledPack {
    pub id: String,
    pub capability: String,
    pub name: String,
    pub version: String,
    pub install_dir: PathBuf,
    pub entrypoint: PathBuf,
    pub installed_at: String,
    pub manifest: PackManifest,
}

impl InstalledPack {
    /// Resolves the absolute or pack-relative entrypoint executable path.
    pub fn resolved_entrypoint(&self) -> PathBuf {
        if self.entrypoint.is_absolute() {
            self.entrypoint.clone()
        } else {
            self.install_dir.join(&self.entrypoint)
        }
    }

    /// Verifies that the entrypoint executable exists on disk.
    pub fn is_healthy(&self) -> bool {
        let entrypoint = self.resolved_entrypoint();
        entrypoint.is_file()
    }
}

/// Local state recording installed packs in the user packs directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackInstallState {
    pub schema_version: String,
    #[serde(default)]
    pub installed: BTreeMap<String, InstalledPack>,
}

impl Default for PackInstallState {
    fn default() -> Self {
        Self {
            schema_version: "1.0.0".to_string(),
            installed: BTreeMap::new(),
        }
    }
}

impl PackInstallState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state_file_path(packs_dir: &Path) -> PathBuf {
        packs_dir.join("installed.json")
    }

    /// Loads the install state from the given packs directory.
    /// Returns default empty state if `installed.json` does not exist.
    pub fn load_from_dir(packs_dir: &Path) -> Result<Self> {
        let path = Self::state_file_path(packs_dir);
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&path)?;
        let state: Self = serde_json::from_str(&content)?;
        Ok(state)
    }

    /// Atomically persists the install state to `installed.json` in `packs_dir`.
    pub fn save_to_dir(&self, packs_dir: &Path) -> Result<()> {
        std::fs::create_dir_all(packs_dir)?;
        let target_path = Self::state_file_path(packs_dir);
        let temp_path = packs_dir.join(format!(
            "installed.json.tmp.{}.{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));

        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&temp_path, json)?;

        // Ensure clean atomic replace
        if target_path.exists() {
            let _ = std::fs::remove_file(&target_path);
        }
        std::fs::rename(&temp_path, &target_path)?;
        Ok(())
    }

    /// Retrieves the installed pack for the specified capability, if any.
    pub fn get_pack(&self, capability: &str) -> Option<&InstalledPack> {
        self.installed.get(capability)
    }

    /// Checks if an installed pack exists for the capability and its entrypoint is intact.
    pub fn is_capability_healthy(&self, capability: &str) -> bool {
        self.get_pack(capability)
            .map(|p| p.is_healthy())
            .unwrap_or(false)
    }

    /// Records or updates an installed pack in this state.
    pub fn record_install(&mut self, pack: InstalledPack) {
        self.installed.insert(pack.capability.clone(), pack);
    }

    /// Removes an installed pack record by capability.
    pub fn record_removal(&mut self, capability: &str) -> Option<InstalledPack> {
        self.installed.remove(capability)
    }
}

/// Resolves the root directory where optional packs and `installed.json` live.
///
/// Priority:
/// 1. `FOLDSHIFT_PACKS_DIR` environment variable.
/// 2. Per-user standard platform directory (no elevated permissions needed).
pub fn packs_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("FOLDSHIFT_PACKS_DIR") {
        let trimmed = dir.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let p = PathBuf::from(local_app_data);
            if !p.as_os_str().is_empty() {
                return p.join("foldshift").join("packs");
            }
        }
        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            let p = PathBuf::from(user_profile);
            if !p.as_os_str().is_empty() {
                return p
                    .join("AppData")
                    .join("Local")
                    .join("foldshift")
                    .join("packs");
            }
        }
        std::env::temp_dir().join("foldshift").join("packs")
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            let p = PathBuf::from(home);
            if !p.as_os_str().is_empty() {
                return p
                    .join("Library")
                    .join("Application Support")
                    .join("foldshift")
                    .join("packs");
            }
        }
        std::env::temp_dir().join("foldshift").join("packs")
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            let p = PathBuf::from(xdg);
            if !p.as_os_str().is_empty() {
                return p.join("foldshift").join("packs");
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            let p = PathBuf::from(home);
            if !p.as_os_str().is_empty() {
                return p
                    .join(".local")
                    .join("share")
                    .join("foldshift")
                    .join("packs");
            }
        }
        std::env::temp_dir().join("foldshift").join("packs")
    }
}

/// Checks whether an installed and healthy pack is available for the given capability.
pub fn is_capability_installed(capability: &str) -> bool {
    let dir = packs_dir();
    match PackInstallState::load_from_dir(&dir) {
        Ok(state) => state.is_capability_healthy(capability),
        Err(_) => false,
    }
}

/// Retrieves the installed pack for a capability if it exists and is healthy.
pub fn get_installed_engine(capability: &str) -> Option<InstalledPack> {
    let dir = packs_dir();
    let state = PackInstallState::load_from_dir(&dir).ok()?;
    let pack = state.get_pack(capability)?.clone();
    if pack.is_healthy() { Some(pack) } else { None }
}

/// Planned or officially supported engine capability packs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackRegistryEntry {
    pub capability: String,
    pub id: String,
    pub name: String,
    pub summary: String,
    pub engine: String,
}

/// Returns the registry of planned and supported capability engine packs.
pub fn known_packs() -> Vec<PackRegistryEntry> {
    vec![
        PackRegistryEntry {
            capability: "pdf".into(),
            id: "qpdf".into(),
            name: "QPDF Engine".into(),
            summary: "PDF structural inspection, page extraction, and merging".into(),
            engine: "qpdf".into(),
        },
        PackRegistryEntry {
            capability: "media".into(),
            id: "media".into(),
            name: "Shared Media Engine".into(),
            summary: "Audio and video stream conversion and inspection".into(),
            engine: "ffmpeg".into(),
        },
    ]
}

/// Comprehensive status of a capability pack (installed, missing, or corrupted).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackStatus {
    pub capability: String,
    pub id: String,
    pub name: String,
    pub summary: String,
    pub installed: bool,
    pub healthy: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_dir: Option<PathBuf>,
}

/// Queries the status of all known capability packs against the default packs directory.
pub fn list_pack_statuses() -> Vec<PackStatus> {
    list_pack_statuses_in(&packs_dir())
}

/// Queries the status of all known capability packs against an explicit packs directory.
pub fn list_pack_statuses_in(packs_dir: &Path) -> Vec<PackStatus> {
    let state = PackInstallState::load_from_dir(packs_dir).unwrap_or_default();

    known_packs()
        .into_iter()
        .map(|known| {
            if let Some(installed) = state.get_pack(&known.capability) {
                let healthy = installed.is_healthy();
                PackStatus {
                    capability: known.capability,
                    id: installed.id.clone(),
                    name: installed.name.clone(),
                    summary: known.summary,
                    installed: true,
                    healthy,
                    version: Some(installed.version.clone()),
                    entrypoint: Some(installed.resolved_entrypoint()),
                    install_dir: Some(installed.install_dir.clone()),
                }
            } else {
                PackStatus {
                    capability: known.capability,
                    id: known.id,
                    name: known.name,
                    summary: known.summary,
                    installed: false,
                    healthy: false,
                    version: None,
                    entrypoint: None,
                    install_dir: None,
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> PackManifest {
        PackManifest {
            schema_version: "1.0.0".into(),
            id: "qpdf".into(),
            name: "QPDF Engine".into(),
            version: "11.9.1".into(),
            capability: "pdf".into(),
            os: std::env::consts::OS.into(),
            arch: std::env::consts::ARCH.into(),
            abi: None,
            core_version_requirement: Some(">=0.1.0".into()),
            entrypoint: PathBuf::from("bin/qpdf.exe"),
            download_bytes: 10_000_000,
            unpacked_bytes: 25_000_000,
            sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".into(),
            file_inventory: vec![
                PackFileEntry {
                    path: PathBuf::from("bin/qpdf.exe"),
                    bytes: 15_000_000,
                    sha256: None,
                },
                PackFileEntry {
                    path: PathBuf::from("share/license.txt"),
                    bytes: 10_000_000,
                    sha256: None,
                },
            ],
            license: "Apache-2.0".into(),
            notice: Some("Notice terms".into()),
            signature: None,
            trusted_key_id: None,
        }
    }

    #[test]
    fn valid_manifest_passes_validation() {
        let manifest = sample_manifest();
        assert!(manifest.validate().is_ok());
        assert!(manifest.verify_host_compatibility().is_ok());
        assert!(manifest.verify_core_compatibility().is_ok());
    }

    #[test]
    fn path_safety_guard_rejects_traversal_and_roots() {
        assert!(is_safe_relative_path(Path::new("bin/qpdf")));
        assert!(is_safe_relative_path(Path::new("bin/qpdf.exe")));
        assert!(is_safe_relative_path(Path::new("lib/sub/file.dll")));

        assert!(!is_safe_relative_path(Path::new("../secret.txt")));
        assert!(!is_safe_relative_path(Path::new("bin/../../secret.txt")));
        assert!(!is_safe_relative_path(Path::new("/bin/qpdf")));
        assert!(!is_safe_relative_path(Path::new("")));
        assert!(!is_safe_relative_path(Path::new("C:\\Windows\\cmd.exe")));
        assert!(!is_safe_relative_path(Path::new("bin/../file")));
    }

    #[test]
    fn manifest_validation_rejects_unsafe_entrypoint() {
        let mut manifest = sample_manifest();
        manifest.entrypoint = PathBuf::from("../bin/qpdf.exe");
        assert!(manifest.validate().is_err());

        manifest.entrypoint = PathBuf::from("/bin/qpdf.exe");
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn manifest_validation_rejects_unsafe_inventory_path() {
        let mut manifest = sample_manifest();
        manifest.file_inventory.push(PackFileEntry {
            path: PathBuf::from("../escaped.txt"),
            bytes: 10,
            sha256: None,
        });
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn manifest_validation_requires_entrypoint_in_inventory() {
        let mut manifest = sample_manifest();
        manifest.entrypoint = PathBuf::from("bin/other.exe");
        let err = manifest.validate().unwrap_err();
        assert!(err.to_string().contains("not present in file_inventory"));
    }

    #[test]
    fn host_compatibility_rejects_foreign_target() {
        let mut manifest = sample_manifest();
        manifest.os = "foreign_os".into();
        assert!(manifest.verify_host_compatibility().is_err());

        let mut manifest = sample_manifest();
        manifest.arch = "foreign_arch".into();
        assert!(manifest.verify_host_compatibility().is_err());
    }

    #[test]
    fn core_version_compatibility_verification() {
        assert!(is_core_version_compatible("0.1.0", "0.1.0"));
        assert!(is_core_version_compatible(">=0.1.0", "0.1.0"));
        assert!(is_core_version_compatible("^0.1", "0.1.0"));
        assert!(is_core_version_compatible("*", "0.1.0"));

        assert!(!is_core_version_compatible("0.2.0", "0.1.0"));
        assert!(!is_core_version_compatible("1.0.0", "0.1.0"));
    }

    #[test]
    fn install_state_save_and_load_lifecycle() {
        let temp_dir = tempfile::tempdir().unwrap();
        let packs_dir = temp_dir.path();

        let initial = PackInstallState::load_from_dir(packs_dir).unwrap();
        assert!(initial.installed.is_empty());

        let bin_path = packs_dir.join("bin");
        std::fs::create_dir_all(&bin_path).unwrap();
        let exe_path = bin_path.join("qpdf.exe");
        std::fs::write(&exe_path, b"mock-executable").unwrap();

        let mut state = PackInstallState::new();
        let installed_pack = InstalledPack {
            id: "qpdf".into(),
            capability: "pdf".into(),
            name: "QPDF Engine".into(),
            version: "11.9.1".into(),
            install_dir: packs_dir.to_path_buf(),
            entrypoint: PathBuf::from("bin/qpdf.exe"),
            installed_at: "2026-10-10T00:00:00Z".into(),
            manifest: sample_manifest(),
        };

        state.record_install(installed_pack);
        state.save_to_dir(packs_dir).unwrap();

        let loaded = PackInstallState::load_from_dir(packs_dir).unwrap();
        assert_eq!(loaded.installed.len(), 1);
        let loaded_pack = loaded.get_pack("pdf").unwrap();
        assert_eq!(loaded_pack.name, "QPDF Engine");
        assert!(loaded_pack.is_healthy());
        assert!(loaded.is_capability_healthy("pdf"));

        // If executable is deleted, health becomes false
        std::fs::remove_file(&exe_path).unwrap();
        assert!(!loaded_pack.is_healthy());
        assert!(!loaded.is_capability_healthy("pdf"));
    }

    #[test]
    fn list_pack_statuses_reflects_installed_and_missing() {
        let temp_dir = tempfile::tempdir().unwrap();
        let packs_dir = temp_dir.path();

        let statuses = list_pack_statuses_in(packs_dir);
        assert_eq!(statuses.len(), 2);
        let pdf_status = statuses.iter().find(|s| s.capability == "pdf").unwrap();
        assert!(!pdf_status.installed);
        assert!(!pdf_status.healthy);

        // Record a mock pack
        let mut state = PackInstallState::new();
        let exe_path = packs_dir.join("qpdf.exe");
        std::fs::write(&exe_path, b"mock").unwrap();

        state.record_install(InstalledPack {
            id: "qpdf".into(),
            capability: "pdf".into(),
            name: "QPDF Engine".into(),
            version: "11.9.1".into(),
            install_dir: packs_dir.to_path_buf(),
            entrypoint: PathBuf::from("qpdf.exe"),
            installed_at: "2026-10-10T00:00:00Z".into(),
            manifest: sample_manifest(),
        });
        state.save_to_dir(packs_dir).unwrap();

        let updated_statuses = list_pack_statuses_in(packs_dir);
        let updated_pdf = updated_statuses
            .iter()
            .find(|s| s.capability == "pdf")
            .unwrap();
        assert!(updated_pdf.installed);
        assert!(updated_pdf.healthy);
        assert_eq!(updated_pdf.version.as_deref(), Some("11.9.1"));
    }
}
