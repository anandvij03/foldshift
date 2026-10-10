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
    #[error("ZIP archive error: {0}")]
    Zip(#[from] zip::result::ZipError),
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
    #[error("checksum mismatch: expected '{expected}', actual '{actual}'")]
    ChecksumMismatch { expected: String, actual: String },
    #[error(
        "archive exceeded size limit: size is {actual} bytes, maximum allowed is {limit} bytes"
    )]
    ExceededSizeLimit { limit: u64, actual: u64 },
    #[error("file size mismatch for '{path}': expected {expected} bytes, found {actual} bytes")]
    PayloadFileMismatch {
        path: PathBuf,
        expected: u64,
        actual: u64,
    },
    #[error("missing file in archive: {0}")]
    MissingFile(String),
    #[error(
        "capability mismatch: archive provides capability '{provided}', but requested '{requested}'"
    )]
    CapabilityMismatch { requested: String, provided: String },
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

/// Maximum allowed archive file size (500 MB) to prevent denial of service.
pub const MAX_ARCHIVE_BYTES: u64 = 500 * 1024 * 1024;

/// Computes lowercase hex-encoded SHA-256 hash of a byte slice.
pub fn compute_sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Computes lowercase hex-encoded SHA-256 hash of a file on disk.
pub fn compute_file_sha256(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = std::io::Read::read(&mut file, &mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Formats current UTC system time as an ISO-8601 UTC timestamp string.
pub fn current_iso_timestamp() -> String {
    let now = std::time::SystemTime::now();
    let duration = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = duration.as_secs();

    let days = total_secs / 86400;
    let rem_secs = total_secs % 86400;
    let hours = rem_secs / 3600;
    let mins = (rem_secs % 3600) / 60;
    let secs = rem_secs % 60;

    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y, m, d, hours, mins, secs
    )
}

/// Options controlling pack installation or updating.
#[derive(Debug, Clone, Default)]
pub struct InstallOptions<'a> {
    /// Expected capability to enforce (e.g. "pdf"). If Some, verified against pack manifest.
    pub expected_capability: Option<&'a str>,
    /// Expected SHA-256 checksum to enforce. If None, checked against companion .sha256 or manifest.
    pub expected_sha256: Option<&'a str>,
}

/// Inspects an offline pack archive (.zip) and reads its `manifest.json` without extracting payload.
pub fn inspect_pack_archive(archive_path: &Path) -> Result<PackManifest> {
    let file = std::fs::File::open(archive_path)?;
    let mut zip = zip::ZipArchive::new(file)?;
    let mut manifest_entry = zip.by_name("manifest.json").map_err(|_| {
        PackError::InvalidManifest("archive does not contain manifest.json at root".into())
    })?;
    let mut content = String::new();
    std::io::Read::read_to_string(&mut manifest_entry, &mut content)?;
    let manifest: PackManifest = serde_json::from_str(&content)?;
    manifest.validate()?;
    Ok(manifest)
}

/// Packages a manifest and files from `payload_dir` into a standard `.zip` pack archive.
/// Returns the calculated SHA-256 checksum of the created archive.
pub fn create_pack_archive(
    manifest: &PackManifest,
    payload_dir: &Path,
    output_zip_path: &Path,
) -> Result<String> {
    use std::io::Write;
    use zip::write::{SimpleFileOptions, ZipWriter};

    manifest.validate()?;

    if let Some(parent) = output_zip_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file = std::fs::File::create(output_zip_path)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // 1. Write manifest.json
    let manifest_bytes = serde_json::to_vec_pretty(manifest)?;
    zip.start_file("manifest.json", options)?;
    zip.write_all(&manifest_bytes)?;

    // 2. Write each file in inventory
    for entry in &manifest.file_inventory {
        let src_path = payload_dir.join(&entry.path);
        if !src_path.is_file() {
            return Err(PackError::MissingFile(format!(
                "source file '{}' not found at '{}'",
                entry.path.display(),
                src_path.display()
            )));
        }
        let rel_name = entry.path.to_string_lossy().replace('\\', "/");
        zip.start_file(rel_name, options)?;
        let data = std::fs::read(&src_path)?;
        zip.write_all(&data)?;
    }

    zip.finish()?;
    compute_file_sha256(output_zip_path)
}

/// Verifies and installs an offline capability pack archive into `packs_dir`.
///
/// Ensures:
/// - Target platform and core version compatibility.
/// - Archive checksum match against explicit expected hash, companion `.sha256`, or manifest.
/// - Strict path traversal rejection (Zip Slip protection).
/// - Decompression bomb limits.
/// - File inventory integrity (size and optional checksums).
/// - Atomic staging and activation, with complete rollback to previous working version on failure.
pub fn install_pack_from_archive(
    archive_path: &Path,
    packs_dir: &Path,
    options: &InstallOptions,
) -> Result<InstalledPack> {
    let metadata = std::fs::metadata(archive_path)?;
    let archive_size = metadata.len();
    if archive_size > MAX_ARCHIVE_BYTES {
        return Err(PackError::ExceededSizeLimit {
            limit: MAX_ARCHIVE_BYTES,
            actual: archive_size,
        });
    }

    let archive_sha = compute_file_sha256(archive_path)?;

    // Check for companion <archive_path>.sha256 if no explicit expected_sha256 passed
    let mut companion_hash: Option<String> = None;
    let companion_path = PathBuf::from(format!("{}.sha256", archive_path.display()));
    if companion_path.is_file()
        && let Ok(content) = std::fs::read_to_string(&companion_path)
        && let Some(first_token) = content.split_whitespace().next()
    {
        companion_hash = Some(first_token.trim().to_ascii_lowercase());
    }

    let expected_hash = options
        .expected_sha256
        .map(|s| s.trim().to_ascii_lowercase())
        .or(companion_hash);

    if let Some(expected) = &expected_hash
        && !archive_sha.eq_ignore_ascii_case(expected)
    {
        return Err(PackError::ChecksumMismatch {
            expected: expected.clone(),
            actual: archive_sha,
        });
    }

    // Open zip archive and read manifest.json
    let file = std::fs::File::open(archive_path)?;
    let mut zip = zip::ZipArchive::new(file)?;

    let manifest = {
        let mut manifest_entry = zip.by_name("manifest.json").map_err(|_| {
            PackError::InvalidManifest("archive does not contain manifest.json at root".into())
        })?;
        let mut content = String::new();
        std::io::Read::read_to_string(&mut manifest_entry, &mut content)?;
        let m: PackManifest = serde_json::from_str(&content)?;
        m
    };

    manifest.validate()?;
    manifest.verify_host_compatibility()?;
    manifest.verify_core_compatibility()?;

    if let Some(req_cap) = options.expected_capability
        && manifest.capability != req_cap
    {
        return Err(PackError::CapabilityMismatch {
            requested: req_cap.to_string(),
            provided: manifest.capability.clone(),
        });
    }

    if expected_hash.is_none()
        && !manifest.sha256.is_empty()
        && manifest.sha256 != "*"
        && !archive_sha.eq_ignore_ascii_case(&manifest.sha256)
    {
        return Err(PackError::ChecksumMismatch {
            expected: manifest.sha256.clone(),
            actual: archive_sha,
        });
    }

    // Pre-flight check: Zip-slip path traversal and anti-zip-bomb limits
    let max_unpacked = std::cmp::max(manifest.unpacked_bytes.saturating_mul(2), 100_000_000);
    let mut total_uncompressed_bytes: u64 = 0;

    for i in 0..zip.len() {
        let entry = zip.by_index(i)?;
        let name = entry.name();
        let path = Path::new(name);

        if !is_safe_relative_path(path) || entry.enclosed_name().is_none() {
            return Err(PackError::UnsafePath(
                path.to_path_buf(),
                "archive entry path escapes root directory or contains traversal sequence".into(),
            ));
        }

        total_uncompressed_bytes = total_uncompressed_bytes.saturating_add(entry.size());
        if total_uncompressed_bytes > max_unpacked {
            return Err(PackError::ExceededSizeLimit {
                limit: max_unpacked,
                actual: total_uncompressed_bytes,
            });
        }
    }

    // Prepare staging directory
    std::fs::create_dir_all(packs_dir)?;
    let staging_dir = packs_dir.join(".staging").join(format!(
        "{}_{}_{}",
        manifest.capability,
        manifest.version,
        std::process::id()
    ));

    if staging_dir.exists() {
        let _ = std::fs::remove_dir_all(&staging_dir);
    }
    std::fs::create_dir_all(&staging_dir)?;

    // Extract files with rollback guarantee
    let extract_outcome: Result<()> = (|| {
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i)?;
            let safe_path = entry.enclosed_name().ok_or_else(|| {
                PackError::UnsafePath(PathBuf::from(entry.name()), "unsafe path".into())
            })?;
            let out_path = staging_dir.join(&safe_path);

            if entry.is_dir() {
                std::fs::create_dir_all(&out_path)?;
            } else {
                if let Some(parent) = out_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut outfile = std::fs::File::create(&out_path)?;
                std::io::copy(&mut entry, &mut outfile)?;
            }
        }

        // Verify entrypoint exists
        let entrypoint_path = staging_dir.join(&manifest.entrypoint);
        if !entrypoint_path.is_file() {
            return Err(PackError::EntrypointMissing(manifest.entrypoint.clone()));
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ =
                std::fs::set_permissions(&entrypoint_path, std::fs::Permissions::from_mode(0o755));
        }

        // Verify file inventory
        for inv_entry in &manifest.file_inventory {
            let file_path = staging_dir.join(&inv_entry.path);
            if !file_path.is_file() {
                return Err(PackError::MissingFile(format!(
                    "expected inventory file '{}' is missing after extraction",
                    inv_entry.path.display()
                )));
            }
            let file_meta = std::fs::metadata(&file_path)?;
            if file_meta.len() != inv_entry.bytes {
                return Err(PackError::PayloadFileMismatch {
                    path: inv_entry.path.clone(),
                    expected: inv_entry.bytes,
                    actual: file_meta.len(),
                });
            }
            if let Some(expected_sha) = &inv_entry.sha256 {
                let actual_sha = compute_file_sha256(&file_path)?;
                if !actual_sha.eq_ignore_ascii_case(expected_sha) {
                    return Err(PackError::ChecksumMismatch {
                        expected: expected_sha.clone(),
                        actual: actual_sha,
                    });
                }
            }
        }

        Ok(())
    })();

    if let Err(err) = extract_outcome {
        let _ = std::fs::remove_dir_all(&staging_dir);
        let staging_root = packs_dir.join(".staging");
        if staging_root.is_dir() {
            let _ = std::fs::remove_dir(&staging_root);
        }
        return Err(err);
    }

    // Atomic activation
    let dest_dir = packs_dir.join(&manifest.capability).join(&manifest.version);
    if let Some(parent) = dest_dir.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if dest_dir.exists()
        && let Err(e) = std::fs::remove_dir_all(&dest_dir)
    {
        let _ = std::fs::remove_dir_all(&staging_dir);
        return Err(PackError::Io(e));
    }

    if let Err(e) = std::fs::rename(&staging_dir, &dest_dir) {
        let _ = std::fs::remove_dir_all(&staging_dir);
        return Err(PackError::Io(e));
    }

    let installed_pack = InstalledPack {
        id: manifest.id.clone(),
        capability: manifest.capability.clone(),
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        install_dir: dest_dir,
        entrypoint: manifest.entrypoint.clone(),
        installed_at: current_iso_timestamp(),
        manifest,
    };

    let mut state = PackInstallState::load_from_dir(packs_dir)?;
    state.record_install(installed_pack.clone());
    state.save_to_dir(packs_dir)?;

    let staging_root = packs_dir.join(".staging");
    if staging_root.is_dir() {
        let _ = std::fs::remove_dir(&staging_root);
    }

    Ok(installed_pack)
}

/// Updates an installed capability pack to a new version from an offline archive.
/// Preserves previous working version if update fails.
pub fn update_pack_from_archive(
    archive_path: &Path,
    packs_dir: &Path,
    options: &InstallOptions,
) -> Result<InstalledPack> {
    let state = PackInstallState::load_from_dir(packs_dir)?;
    let old_pack = if let Some(req_cap) = options.expected_capability {
        state.get_pack(req_cap).cloned()
    } else {
        let manifest = inspect_pack_archive(archive_path)?;
        state.get_pack(&manifest.capability).cloned()
    };

    let new_pack = install_pack_from_archive(archive_path, packs_dir, options)?;

    // If previous version was in a different directory, cleanly remove it
    if let Some(old) = old_pack
        && old.install_dir != new_pack.install_dir
        && old.install_dir.exists()
    {
        let _ = std::fs::remove_dir_all(&old.install_dir);
    }

    Ok(new_pack)
}

/// Removes an installed capability pack and updates `installed.json`.
/// Preserves user files and other capabilities.
pub fn remove_pack(capability: &str, packs_dir: &Path) -> Result<InstalledPack> {
    let mut state = PackInstallState::load_from_dir(packs_dir)?;
    let pack = state
        .record_removal(capability)
        .ok_or_else(|| PackError::NotInstalled(capability.to_string()))?;

    if pack.install_dir.exists() {
        std::fs::remove_dir_all(&pack.install_dir)?;
    }

    let cap_dir = packs_dir.join(capability);
    if cap_dir.is_dir()
        && let Ok(mut entries) = std::fs::read_dir(&cap_dir)
        && entries.next().is_none()
    {
        let _ = std::fs::remove_dir(&cap_dir);
    }

    state.save_to_dir(packs_dir)?;
    Ok(pack)
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

    #[test]
    fn sha256_computation_and_file_hashing() {
        let temp = tempfile::tempdir().unwrap();
        let file_path = temp.path().join("test.txt");
        std::fs::write(&file_path, b"hello foldshift pack").unwrap();

        let expected = compute_sha256(b"hello foldshift pack");
        let actual = compute_file_sha256(&file_path).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn timestamp_generation_validity() {
        let ts = current_iso_timestamp();
        assert!(ts.ends_with('Z'));
        assert!(ts.contains('T'));
        assert_eq!(ts.len(), 20); // YYYY-MM-DDTHH:MM:SSZ
    }

    fn build_test_pack_archive(
        payload_dir: &Path,
        zip_path: &Path,
        version: &str,
    ) -> (PackManifest, String) {
        let bin_dir = payload_dir.join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        let exe_name = if cfg!(windows) { "qpdf.exe" } else { "qpdf" };
        let exe_path = bin_dir.join(exe_name);
        std::fs::write(&exe_path, b"mock-executable-binary").unwrap();

        let share_dir = payload_dir.join("share");
        std::fs::create_dir_all(&share_dir).unwrap();
        let lic_path = share_dir.join("LICENSE");
        std::fs::write(&lic_path, b"Apache-2.0 License Text").unwrap();

        let exe_bytes = std::fs::metadata(&exe_path).unwrap().len();
        let lic_bytes = std::fs::metadata(&lic_path).unwrap().len();

        let exe_rel = PathBuf::from("bin").join(exe_name);
        let lic_rel = PathBuf::from("share/LICENSE");

        let manifest = PackManifest {
            schema_version: "1.0.0".into(),
            id: "qpdf".into(),
            name: "QPDF Engine".into(),
            version: version.into(),
            capability: "pdf".into(),
            os: std::env::consts::OS.into(),
            arch: std::env::consts::ARCH.into(),
            abi: None,
            core_version_requirement: Some(">=0.1.0".into()),
            entrypoint: exe_rel.clone(),
            download_bytes: 100,
            unpacked_bytes: exe_bytes + lic_bytes,
            sha256: "*".into(),
            file_inventory: vec![
                PackFileEntry {
                    path: exe_rel,
                    bytes: exe_bytes,
                    sha256: Some(compute_file_sha256(&exe_path).unwrap()),
                },
                PackFileEntry {
                    path: lic_rel,
                    bytes: lic_bytes,
                    sha256: Some(compute_file_sha256(&lic_path).unwrap()),
                },
            ],
            license: "Apache-2.0".into(),
            notice: Some("Pack license terms".into()),
            signature: None,
            trusted_key_id: None,
        };

        // Create archive with wildcard sha
        let zip_sha = create_pack_archive(&manifest, payload_dir, zip_path).unwrap();
        (manifest, zip_sha)
    }

    #[test]
    fn inspect_pack_archive_reads_manifest_without_installing() {
        let temp = tempfile::tempdir().unwrap();
        let payload_dir = temp.path().join("payload");
        let zip_path = temp.path().join("test_pack.zip");

        let (manifest, _) = build_test_pack_archive(&payload_dir, &zip_path, "11.9.1");

        let inspected = inspect_pack_archive(&zip_path).unwrap();
        assert_eq!(inspected.id, manifest.id);
        assert_eq!(inspected.capability, "pdf");
        assert_eq!(inspected.version, "11.9.1");
        assert_eq!(inspected.file_inventory.len(), 2);
    }

    #[test]
    fn install_pack_from_archive_lifecycle_and_verification() {
        let temp = tempfile::tempdir().unwrap();
        let payload_dir = temp.path().join("payload");
        let zip_path = temp.path().join("qpdf_pack.zip");
        let packs_dir = temp.path().join("packs");

        let (manifest, zip_sha) = build_test_pack_archive(&payload_dir, &zip_path, "11.9.1");

        // Install pack
        let options = InstallOptions {
            expected_capability: Some("pdf"),
            expected_sha256: Some(&zip_sha),
        };
        let installed = install_pack_from_archive(&zip_path, &packs_dir, &options).unwrap();
        assert_eq!(installed.capability, "pdf");
        assert_eq!(installed.version, "11.9.1");
        assert!(installed.is_healthy());

        // Verify state persistence
        let state = PackInstallState::load_from_dir(&packs_dir).unwrap();
        assert!(state.is_capability_healthy("pdf"));
        let loaded = state.get_pack("pdf").unwrap();
        assert_eq!(loaded.name, "QPDF Engine");
        assert_eq!(loaded.version, "11.9.1");

        // Verify files exist in versioned path
        let dest_dir = packs_dir.join("pdf").join("11.9.1");
        assert!(dest_dir.join(&manifest.entrypoint).is_file());
        assert!(dest_dir.join("share/LICENSE").is_file());
    }

    #[test]
    fn install_pack_rejects_checksum_mismatch() {
        let temp = tempfile::tempdir().unwrap();
        let payload_dir = temp.path().join("payload");
        let zip_path = temp.path().join("qpdf_pack.zip");
        let packs_dir = temp.path().join("packs");

        let (_manifest, _zip_sha) = build_test_pack_archive(&payload_dir, &zip_path, "11.9.1");

        let options = InstallOptions {
            expected_capability: Some("pdf"),
            expected_sha256: Some(
                "bad_hash_0000000000000000000000000000000000000000000000000000000000000",
            ),
        };
        let res = install_pack_from_archive(&zip_path, &packs_dir, &options);
        assert!(res.is_err());
        match res.unwrap_err() {
            PackError::ChecksumMismatch { .. } => {}
            other => panic!("expected ChecksumMismatch, got {:?}", other),
        }

        // Staging directory was cleaned up and nothing is installed
        assert!(!packs_dir.join(".staging").exists());
        let state = PackInstallState::load_from_dir(&packs_dir).unwrap();
        assert!(!state.is_capability_healthy("pdf"));
    }

    #[test]
    fn install_pack_rejects_unsafe_zip_slip_path() {
        use std::io::Write;
        use zip::write::{SimpleFileOptions, ZipWriter};

        let temp = tempfile::tempdir().unwrap();
        let zip_path = temp.path().join("zip_slip.zip");
        let packs_dir = temp.path().join("packs");

        let mut manifest = sample_manifest();
        manifest.sha256 = "*".into();

        let file = std::fs::File::create(&zip_path).unwrap();
        let mut zip = ZipWriter::new(file);
        let opts = SimpleFileOptions::default();

        let manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap();
        zip.start_file("manifest.json", opts).unwrap();
        zip.write_all(&manifest_bytes).unwrap();

        // Inject malicious Zip Slip entry
        zip.start_file("../outside_escape.txt", opts).unwrap();
        zip.write_all(b"malicious escape content").unwrap();
        zip.finish().unwrap();

        let options = InstallOptions::default();
        let res = install_pack_from_archive(&zip_path, &packs_dir, &options);
        assert!(res.is_err());
        match res.unwrap_err() {
            PackError::UnsafePath(p, _) => {
                assert!(p.to_string_lossy().contains(".."));
            }
            other => panic!("expected UnsafePath, got {:?}", other),
        }

        // Verify malicious file was NOT written outside
        assert!(!temp.path().join("outside_escape.txt").exists());
        assert!(!packs_dir.join(".staging").exists());
    }

    #[test]
    fn install_pack_rejects_payload_size_mismatch() {
        use std::io::Write;
        use zip::write::{SimpleFileOptions, ZipWriter};

        let temp = tempfile::tempdir().unwrap();
        let zip_path = temp.path().join("tampered_payload.zip");
        let packs_dir = temp.path().join("packs");

        let mut manifest = sample_manifest();
        manifest.sha256 = "*".into();
        manifest.entrypoint = PathBuf::from("bin/qpdf.exe");
        manifest.file_inventory = vec![PackFileEntry {
            path: PathBuf::from("bin/qpdf.exe"),
            bytes: 500, // Manifest states 500 bytes
            sha256: None,
        }];

        let file = std::fs::File::create(&zip_path).unwrap();
        let mut zip = ZipWriter::new(file);
        let opts = SimpleFileOptions::default();

        let manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap();
        zip.start_file("manifest.json", opts).unwrap();
        zip.write_all(&manifest_bytes).unwrap();

        // Write only 10 bytes instead of declared 500 bytes
        zip.start_file("bin/qpdf.exe", opts).unwrap();
        zip.write_all(b"short bytes").unwrap();
        zip.finish().unwrap();

        let options = InstallOptions::default();
        let res = install_pack_from_archive(&zip_path, &packs_dir, &options);
        assert!(res.is_err());
        match res.unwrap_err() {
            PackError::PayloadFileMismatch {
                path,
                expected,
                actual,
            } => {
                assert_eq!(path, PathBuf::from("bin/qpdf.exe"));
                assert_eq!(expected, 500);
                assert_ne!(actual, 500);
            }
            other => panic!("expected PayloadFileMismatch, got {:?}", other),
        }

        // Staging cleaned up
        assert!(!packs_dir.join(".staging").exists());
    }

    #[test]
    fn rollback_on_failed_update_preserves_working_version() {
        let temp = tempfile::tempdir().unwrap();
        let payload1 = temp.path().join("payload1");
        let zip1 = temp.path().join("pack_v1.zip");
        let packs_dir = temp.path().join("packs");

        // 1. Install working v1
        let (_manifest1, sha1) = build_test_pack_archive(&payload1, &zip1, "1.0.0");
        let opt1 = InstallOptions {
            expected_capability: Some("pdf"),
            expected_sha256: Some(&sha1),
        };
        let installed1 = install_pack_from_archive(&zip1, &packs_dir, &opt1).unwrap();
        assert_eq!(installed1.version, "1.0.0");
        assert!(installed1.is_healthy());

        // 2. Attempt update to broken v2 (corrupted archive)
        let broken_zip = temp.path().join("broken_v2.zip");
        std::fs::write(&broken_zip, b"corrupted non-zip payload").unwrap();

        let opt2 = InstallOptions {
            expected_capability: Some("pdf"),
            expected_sha256: None,
        };
        let update_res = update_pack_from_archive(&broken_zip, &packs_dir, &opt2);
        assert!(update_res.is_err());

        // 3. Verify original v1 is completely intact and healthy!
        let state = PackInstallState::load_from_dir(&packs_dir).unwrap();
        assert!(state.is_capability_healthy("pdf"));
        let cur = state.get_pack("pdf").unwrap();
        assert_eq!(cur.version, "1.0.0");
        assert!(cur.is_healthy());
    }

    #[test]
    fn update_and_remove_pack_lifecycle() {
        let temp = tempfile::tempdir().unwrap();
        let payload1 = temp.path().join("payload1");
        let zip1 = temp.path().join("pack_v1.zip");
        let payload2 = temp.path().join("payload2");
        let zip2 = temp.path().join("pack_v2.zip");
        let packs_dir = temp.path().join("packs");

        // Install v1
        let (_manifest1, _sha1) = build_test_pack_archive(&payload1, &zip1, "1.0.0");
        let opt = InstallOptions::default();
        let inst1 = install_pack_from_archive(&zip1, &packs_dir, &opt).unwrap();
        assert_eq!(inst1.version, "1.0.0");

        // Update to v2
        let (_manifest2, _sha2) = build_test_pack_archive(&payload2, &zip2, "2.0.0");
        let inst2 = update_pack_from_archive(&zip2, &packs_dir, &opt).unwrap();
        assert_eq!(inst2.version, "2.0.0");
        assert!(inst2.is_healthy());

        let state = PackInstallState::load_from_dir(&packs_dir).unwrap();
        assert_eq!(state.get_pack("pdf").unwrap().version, "2.0.0");
        // Old v1 folder removed to save disk space
        assert!(!packs_dir.join("pdf").join("1.0.0").exists());
        // New v2 folder exists
        assert!(packs_dir.join("pdf").join("2.0.0").exists());

        // Remove pack
        let removed = remove_pack("pdf", &packs_dir).unwrap();
        assert_eq!(removed.capability, "pdf");
        assert_eq!(removed.version, "2.0.0");

        // State reflects removal
        let state_after = PackInstallState::load_from_dir(&packs_dir).unwrap();
        assert!(!state_after.is_capability_healthy("pdf"));
        assert!(state_after.get_pack("pdf").is_none());
        assert!(!packs_dir.join("pdf").exists());

        // Removing already-removed pack returns NotInstalled
        let err = remove_pack("pdf", &packs_dir).unwrap_err();
        match err {
            PackError::NotInstalled(c) => assert_eq!(c, "pdf"),
            other => panic!("expected NotInstalled, got {:?}", other),
        }
    }
}
