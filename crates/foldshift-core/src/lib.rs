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

impl Inspection {
    pub fn new(
        path: impl Into<PathBuf>,
        format: impl Into<String>,
        media_kind: MediaKind,
        bytes: u64,
    ) -> Self {
        Self {
            path: path.into(),
            format: format.into(),
            media_kind,
            bytes,
            properties: BTreeMap::new(),
        }
    }

    pub fn with_property(mut self, key: impl Into<String>, value: Value) -> Self {
        self.properties.insert(key.into(), value);
        self
    }
}

pub mod batch;
pub mod pack;

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
    #[error("pack error: {0}")]
    Pack(#[from] pack::PackError),
}

pub type Result<T> = std::result::Result<T, FcError>;

pub fn operations() -> Vec<OperationInfo> {
    operations_with(|cap| cap == "core")
}

pub fn operations_with(mut is_available: impl FnMut(&str) -> bool) -> Vec<OperationInfo> {
    vec![
        OperationInfo {
            id: "image.inspect".into(),
            summary: "Inspect an image without modifying it".into(),
            capability: "image".into(),
            available: is_available("image"),
        },
        OperationInfo {
            id: "image.convert".into(),
            summary: "Decode and re-encode an image".into(),
            capability: "image".into(),
            available: is_available("image"),
        },
        OperationInfo {
            id: "image.resize".into(),
            summary: "Resize an image while preserving aspect ratio".into(),
            capability: "image".into(),
            available: is_available("image"),
        },
        OperationInfo {
            id: "batch.convert".into(),
            summary: "Batch convert multiple files sequentially with progress reporting".into(),
            capability: "image".into(),
            available: is_available("image"),
        },
        OperationInfo {
            id: "batch.resize".into(),
            summary: "Batch resize multiple files sequentially with progress reporting".into(),
            capability: "image".into(),
            available: is_available("image"),
        },
        OperationInfo {
            id: "pdf.inspect".into(),
            summary: "Inspect PDF document structure and page count".into(),
            capability: "pdf".into(),
            available: is_available("pdf"),
        },
        OperationInfo {
            id: "pdf.merge".into(),
            summary: "Merge PDF documents".into(),
            capability: "pdf".into(),
            available: is_available("pdf"),
        },
        OperationInfo {
            id: "pdf.extract".into(),
            summary: "Extract page ranges from a PDF document".into(),
            capability: "pdf".into(),
            available: is_available("pdf"),
        },
        OperationInfo {
            id: "audio.convert".into(),
            summary: "Convert an audio stream".into(),
            capability: "media".into(),
            available: is_available("media"),
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

    #[test]
    fn inspection_builder_and_serialization() {
        let insp = Inspection::new(PathBuf::from("test.png"), "PNG", MediaKind::Image, 1024)
            .with_property("width", serde_json::json!(100))
            .with_property("height", serde_json::json!(200));

        assert_eq!(insp.format, "PNG");
        assert_eq!(insp.media_kind, MediaKind::Image);
        assert_eq!(insp.bytes, 1024);
        assert_eq!(insp.properties.get("width").unwrap(), &100);
        assert_eq!(insp.properties.get("height").unwrap(), &200);

        let json = serde_json::to_string(&insp).unwrap();
        assert!(json.contains("\"media_kind\":\"image\""));
        assert!(json.contains("\"width\":100"));
    }

    #[test]
    fn operations_with_filters_availability() {
        let none = operations();
        assert!(!none.iter().any(|op| op.available));

        let with_image = operations_with(|cap| cap == "image");
        let image_ops: Vec<_> = with_image
            .iter()
            .filter(|op| op.capability == "image")
            .collect();
        assert_eq!(image_ops.len(), 5);
        assert!(image_ops.iter().all(|op| op.available));

        let non_image_ops: Vec<_> = with_image
            .iter()
            .filter(|op| op.capability != "image")
            .collect();
        assert!(non_image_ops.iter().all(|op| !op.available));
    }
}
