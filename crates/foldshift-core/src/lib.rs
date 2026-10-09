use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaKind {
    Image,
    Pdf,
    Audio,
    Video,
    Archive,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Format {
    pub name: String,
    pub extension: String,
    pub media_kind: MediaKind,
}

impl Format {
    pub fn new(
        name: impl Into<String>,
        extension: impl Into<String>,
        media_kind: MediaKind,
    ) -> Self {
        Self {
            name: name.into(),
            extension: extension.into(),
            media_kind,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub operation: String,
    pub inputs: Vec<PathBuf>,
    pub output: Option<PathBuf>,
    #[serde(default)]
    pub options: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationInfo {
    pub id: String,
    pub summary: String,
    pub capability: String,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inspection {
    pub path: PathBuf,
    pub format: String,
    pub media_kind: MediaKind,
    pub bytes: u64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, Value>,
}

#[derive(Debug, Error)]
pub enum FcError {
    #[error("input does not exist: {0}")]
    InputMissing(PathBuf),
    #[error("output already exists (pass --force to replace it): {0}")]
    OutputExists(PathBuf),
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),
    #[error("unsupported content: {0}")]
    UnsupportedContent(String),
    #[error("invalid option: {0}")]
    InvalidOption(String),
    #[error("operation is unavailable: {0}")]
    CapabilityUnavailable(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("conversion failed: {0}")]
    Conversion(String),
}

pub type Result<T> = std::result::Result<T, FcError>;

pub fn operations() -> Vec<OperationInfo> {
    vec![
        OperationInfo {
            id: "image.inspect".into(),
            summary: "Inspect an image without modifying it".into(),
            capability: "image".into(),
            available: true,
        },
        OperationInfo {
            id: "image.convert".into(),
            summary: "Decode and re-encode an image".into(),
            capability: "image".into(),
            available: true,
        },
        OperationInfo {
            id: "image.resize".into(),
            summary: "Resize an image while preserving aspect ratio".into(),
            capability: "image".into(),
            available: true,
        },
        OperationInfo {
            id: "pdf.merge".into(),
            summary: "Merge PDF documents".into(),
            capability: "pdf".into(),
            available: false,
        },
        OperationInfo {
            id: "audio.convert".into(),
            summary: "Convert an audio stream".into(),
            capability: "media".into(),
            available: false,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_ids_are_unique() {
        let items = operations();
        let mut ids: Vec<_> = items.iter().map(|item| item.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), items.len());
    }
}
