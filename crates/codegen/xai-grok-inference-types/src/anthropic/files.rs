//! Anthropic Files API wire types (beta `files-api-2025-04-14`).
//!
//! Upload accepts in-memory bytes only; the client never auto-reads local
//! filesystem paths. File bytes must not appear in errors, Debug, or logs.

use serde::{Deserialize, Serialize};
use std::fmt;

/// In-memory file payload for multipart upload.
///
/// Debug omits bytes so secrets or document contents never leak through logs.
#[derive(Clone)]
pub struct FileUploadSource {
    pub filename: String,
    pub mime_type: Option<String>,
    pub bytes: Vec<u8>,
}

impl FileUploadSource {
    pub fn new(filename: impl Into<String>, bytes: Vec<u8>) -> Self {
        Self {
            filename: filename.into(),
            mime_type: None,
            bytes,
        }
    }

    pub fn with_mime_type(mut self, mime: impl Into<String>) -> Self {
        self.mime_type = Some(mime.into());
        self
    }

    pub fn size(&self) -> usize {
        self.bytes.len()
    }
}

impl fmt::Debug for FileUploadSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileUploadSource")
            .field("filename", &self.filename)
            .field("mime_type", &self.mime_type)
            .field("bytes_len", &self.bytes.len())
            .finish()
    }
}

/// Query parameters for listing files (cursor pagination).
#[derive(Debug, Clone, Default, Serialize)]
pub struct ListFilesParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// File metadata from upload / list / retrieve.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileMetadata {
    pub id: String,
    pub filename: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downloadable: Option<bool>,
}

/// Cursor-paginated file list response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileListPage {
    pub data: Vec<FileMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_id: Option<String>,
    #[serde(default)]
    pub has_more: bool,
}

/// Response from `DELETE /v1/files/{id}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteFileResponse {
    pub id: String,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(default)]
    pub deleted: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_upload_source_debug_omits_bytes() {
        let src = FileUploadSource::new("secret.pdf", b"TOPSECRET".to_vec());
        let dbg = format!("{src:?}");
        assert!(!dbg.contains("TOPSECRET"));
        assert!(dbg.contains("bytes_len"));
        assert!(dbg.contains("secret.pdf"));
    }

    #[test]
    fn file_metadata_parses() {
        let raw = r#"{
            "id": "file_1",
            "filename": "doc.pdf",
            "mime_type": "application/pdf",
            "size_bytes": 1024,
            "created_at": "2025-04-15T18:37:24.100435Z",
            "type": "file",
            "downloadable": true
        }"#;
        let meta: FileMetadata = serde_json::from_str(raw).unwrap();
        assert_eq!(meta.id, "file_1");
        assert_eq!(meta.size_bytes, Some(1024));
        assert_eq!(meta.downloadable, Some(true));
    }
}
