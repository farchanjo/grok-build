//! Path-safe asset keys, key prefixes, and media types.
//!
//! Validation is strict so the local backend needs no canonicalization for
//! containment: a key that survives [`AssetKey::parse`] cannot escape the root
//! through `..`, cannot produce an empty path segment, and cannot collide with
//! the reserved `_meta/` namespace that holds visibility sidecars.
//!
//! Accepted alphabet for every segment: `[A-Za-z0-9._~-]`, with `/` as the
//! separator. ASCII only — a non-ASCII key is rejected rather than
//! percent-encoded, so the on-disk path and the object key always agree.

use std::fmt;
use std::path::Path;
use std::str::FromStr;

/// Longest accepted key, in bytes.
pub const MAX_KEY_BYTES: usize = 1_024;
/// Longest accepted single segment, in bytes.
pub const MAX_SEGMENT_BYTES: usize = 255;
/// Longest accepted content type, in bytes.
pub const MAX_CONTENT_TYPE_BYTES: usize = 255;
/// First path segment reserved for sidecars (`_meta/<key>.visibility.json`).
pub const RESERVED_META_SEGMENT: &str = "_meta";
/// Fallback media type when an extension is unknown or absent.
pub const DEFAULT_CONTENT_TYPE: &str = "application/octet-stream";

/// Why a key or prefix was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeyError {
    /// The input was empty where a value is required.
    Empty,
    /// The key exceeded [`MAX_KEY_BYTES`].
    TooLong { len: usize, max: usize },
    /// A single segment exceeded [`MAX_SEGMENT_BYTES`].
    SegmentTooLong {
        segment: String,
        len: usize,
        max: usize,
    },
    /// Two separators in a row, or a leading/trailing separator.
    EmptySegment { index: usize },
    /// A segment was exactly `.`.
    DotSegment { index: usize },
    /// A segment was exactly `..`.
    DotDotSegment { index: usize },
    /// The first segment was the reserved `_meta` namespace.
    ReservedMetaSegment,
    /// A character outside ASCII.
    NonAscii { ch: char, index: usize },
    /// An ASCII character outside the accepted alphabet.
    InvalidChar { ch: char, index: usize },
    /// An `s3://` / `gs://` URL carried no object path.
    MissingObjectPath { scheme: String },
}

impl KeyError {
    /// Stable snake_case code.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Empty => "key_empty",
            Self::TooLong { .. } => "key_too_long",
            Self::SegmentTooLong { .. } => "key_segment_too_long",
            Self::EmptySegment { .. } => "key_empty_segment",
            Self::DotSegment { .. } => "key_dot_segment",
            Self::DotDotSegment { .. } => "key_dot_dot_segment",
            Self::ReservedMetaSegment => "key_reserved_meta_segment",
            Self::NonAscii { .. } => "key_non_ascii",
            Self::InvalidChar { .. } => "key_invalid_char",
            Self::MissingObjectPath { .. } => "key_missing_object_path",
        }
    }
}

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("key must not be empty"),
            Self::TooLong { len, max } => {
                write!(f, "key is {len} bytes, over the {max}-byte limit")
            }
            Self::SegmentTooLong { segment, len, max } => {
                write!(
                    f,
                    "segment `{segment}` is {len} bytes, over the {max}-byte limit"
                )
            }
            Self::EmptySegment { index } => {
                write!(f, "segment {index} is empty (leading, trailing, or `//`)")
            }
            Self::DotSegment { index } => write!(f, "segment {index} is `.`"),
            Self::DotDotSegment { index } => write!(f, "segment {index} is `..`"),
            Self::ReservedMetaSegment => {
                write!(
                    f,
                    "first segment must not be the reserved `{RESERVED_META_SEGMENT}`"
                )
            }
            Self::NonAscii { ch, index } => {
                write!(f, "byte {index} is non-ASCII (`{ch}`)")
            }
            Self::InvalidChar { ch, index } => {
                write!(f, "byte {index} is `{ch}`, outside [A-Za-z0-9._~-] and `/`")
            }
            Self::MissingObjectPath { scheme } => {
                write!(f, "`{scheme}://bucket` carries no object path")
            }
        }
    }
}

impl std::error::Error for KeyError {}

impl From<KeyError> for super::AssetError {
    fn from(err: KeyError) -> Self {
        Self::InvalidKey {
            key: String::new(),
            reason: err.to_string(),
        }
    }
}

/// Why a content type was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ContentTypeError {
    Empty,
    TooLong { len: usize, max: usize },
    MissingSlash,
    EmptyToken { part: &'static str },
    InvalidToken { token: String, ch: char },
    NonAscii { ch: char },
}

impl ContentTypeError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Empty => "content_type_empty",
            Self::TooLong { .. } => "content_type_too_long",
            Self::MissingSlash => "content_type_missing_slash",
            Self::EmptyToken { .. } => "content_type_empty_token",
            Self::InvalidToken { .. } => "content_type_invalid_token",
            Self::NonAscii { .. } => "content_type_non_ascii",
        }
    }
}

impl fmt::Display for ContentTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("content type must not be empty"),
            Self::TooLong { len, max } => {
                write!(f, "content type is {len} bytes, over the {max}-byte limit")
            }
            Self::MissingSlash => f.write_str("content type must be `type/subtype`"),
            Self::EmptyToken { part } => write!(f, "content type {part} must not be empty"),
            Self::InvalidToken { token, ch } => {
                write!(f, "content type token `{token}` contains `{ch}`")
            }
            Self::NonAscii { ch } => write!(f, "content type is non-ASCII (`{ch}`)"),
        }
    }
}

impl std::error::Error for ContentTypeError {}

impl From<ContentTypeError> for super::AssetError {
    fn from(err: ContentTypeError) -> Self {
        Self::InvalidContentType {
            value: String::new(),
            reason: err.to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Shared path validation
// ---------------------------------------------------------------------------

/// Whether the trailing `/` of a prefix is allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PathShape {
    Key,
    Prefix,
}

/// Strip a leading `s3://` / `gs://` bucket qualifier.
///
/// Returns the borrowed remainder so the caller keeps a zero-copy path in the
/// common (unqualified) case.
fn strip_bucket_scheme(input: &str) -> Result<&str, KeyError> {
    let Some(rest) = input
        .strip_prefix("s3://")
        .or_else(|| input.strip_prefix("gs://"))
    else {
        return Ok(input);
    };
    let scheme = input[..input.len() - rest.len()].trim_end_matches("://");
    match rest.split_once('/') {
        Some((_bucket, object)) if !object.is_empty() => Ok(object),
        _ => Err(KeyError::MissingObjectPath {
            scheme: scheme.to_owned(),
        }),
    }
}

fn validate_path(path: &str, shape: PathShape, allow_meta: bool) -> Result<(), KeyError> {
    if path.is_empty() {
        return Ok(());
    }
    if path.len() > MAX_KEY_BYTES {
        return Err(KeyError::TooLong {
            len: path.len(),
            max: MAX_KEY_BYTES,
        });
    }
    if path.starts_with('/') {
        return Err(KeyError::EmptySegment { index: 0 });
    }

    let trailing_slash = path.ends_with('/');
    if trailing_slash && shape == PathShape::Key {
        return Err(KeyError::EmptySegment {
            index: path.split('/').count() - 1,
        });
    }

    let body = if trailing_slash {
        &path[..path.len() - 1]
    } else {
        path
    };

    for (index, segment) in body.split('/').enumerate() {
        if segment.is_empty() {
            return Err(KeyError::EmptySegment { index });
        }
        if segment == "." {
            return Err(KeyError::DotSegment { index });
        }
        if segment == ".." {
            return Err(KeyError::DotDotSegment { index });
        }
        if segment.len() > MAX_SEGMENT_BYTES {
            return Err(KeyError::SegmentTooLong {
                segment: segment.to_owned(),
                len: segment.len(),
                max: MAX_SEGMENT_BYTES,
            });
        }
        if index == 0 && segment == RESERVED_META_SEGMENT && !allow_meta {
            return Err(KeyError::ReservedMetaSegment);
        }
        for (offset, ch) in segment.char_indices() {
            if !ch.is_ascii() {
                return Err(KeyError::NonAscii {
                    ch,
                    index: index + offset,
                });
            }
            if !(ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '~' | '-')) {
                return Err(KeyError::InvalidChar {
                    ch,
                    index: index + offset,
                });
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// AssetKey
// ---------------------------------------------------------------------------

/// A validated, path-safe object key.
///
/// `parse` accepts a bare key (`uploads/report.pdf`) and strips a leading
/// `s3://bucket/` or `gs://bucket/` qualifier so the same string works whether
/// it came from config, a tool argument, or a copied object URL.
///
/// `parse_physical` additionally allows a leading `_meta/` segment, which is
/// how [`super::AssetStore::list`] reports sidecar objects.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AssetKey(String);

impl AssetKey {
    /// Parse a logical key, rejecting the reserved `_meta/` namespace.
    pub fn parse(input: &str) -> Result<Self, KeyError> {
        let path = strip_bucket_scheme(input)?;
        validate_path(path, PathShape::Key, false)?;
        if path.is_empty() {
            return Err(KeyError::Empty);
        }
        Ok(Self(path.to_owned()))
    }

    /// Parse a physical key that may live under the reserved `_meta/`
    /// namespace (used when listing sidecars).
    pub fn parse_physical(input: &str) -> Result<Self, KeyError> {
        let path = strip_bucket_scheme(input)?;
        validate_path(path, PathShape::Key, true)?;
        if path.is_empty() {
            return Err(KeyError::Empty);
        }
        Ok(Self(path.to_owned()))
    }

    /// Join a prefix and a relative key.
    pub fn with_prefix(prefix: &AssetPrefix, rest: &str) -> Result<Self, KeyError> {
        if prefix.is_empty() {
            return Self::parse(rest);
        }
        Self::parse(&format!("{}{rest}", prefix.as_str()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }

    /// Path segments, always non-empty.
    pub fn segments(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }

    /// Final segment.
    pub fn file_name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or(&self.0)
    }

    /// Lowercased extension of the final segment, when present.
    pub fn extension(&self) -> Option<String> {
        let name = self.file_name();
        let (_, ext) = name.rsplit_once('.')?;
        if ext.is_empty() {
            return None;
        }
        Some(ext.to_ascii_lowercase())
    }

    /// Parent prefix, or `None` for a top-level key.
    pub fn parent(&self) -> Option<AssetPrefix> {
        let (parent, _) = self.0.rsplit_once('/')?;
        AssetPrefix::parse(&format!("{parent}/")).ok()
    }

    /// True when `self` sits under `prefix`.
    pub fn starts_with(&self, prefix: &AssetPrefix) -> bool {
        prefix.is_empty() || self.0.starts_with(prefix.as_str())
    }

    /// The part of `self` below `prefix`.
    pub fn strip_prefix(&self, prefix: &AssetPrefix) -> Option<&str> {
        if prefix.is_empty() {
            return Some(&self.0);
        }
        self.0.strip_prefix(prefix.as_str())
    }

    /// True when the key lives in the reserved sidecar namespace.
    pub fn is_physical_meta(&self) -> bool {
        self.0.starts_with(RESERVED_META_SEGMENT)
    }
}

impl fmt::Display for AssetKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for AssetKey {
    type Err = KeyError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

// ---------------------------------------------------------------------------
// AssetPrefix
// ---------------------------------------------------------------------------

/// A validated key prefix. May be empty; may end with `/`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct AssetPrefix(String);

impl AssetPrefix {
    pub fn parse(input: &str) -> Result<Self, KeyError> {
        let path = strip_bucket_scheme(input)?;
        validate_path(path, PathShape::Prefix, true)?;
        Ok(Self(path.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Normalized form with exactly one trailing `/` (empty stays empty).
    pub fn normalized(&self) -> String {
        if self.0.is_empty() || self.0.ends_with('/') {
            self.0.clone()
        } else {
            format!("{}/", self.0)
        }
    }

    /// True when `self` sits under `other`.
    pub fn starts_with(&self, other: &AssetPrefix) -> bool {
        other.is_empty() || self.0.starts_with(other.as_str())
    }
}

impl fmt::Display for AssetPrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for AssetPrefix {
    type Err = KeyError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

// ---------------------------------------------------------------------------
// ContentType
// ---------------------------------------------------------------------------

/// A validated RFC 7231 media type, with optional parameters.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContentType(String);

impl ContentType {
    /// Validate a `type/subtype[; params]` media type.
    pub fn parse(input: &str) -> Result<Self, ContentTypeError> {
        let value = input.trim();
        if value.is_empty() {
            return Err(ContentTypeError::Empty);
        }
        if value.len() > MAX_CONTENT_TYPE_BYTES {
            return Err(ContentTypeError::TooLong {
                len: value.len(),
                max: MAX_CONTENT_TYPE_BYTES,
            });
        }
        if let Some(bad) = value.chars().find(|c| !c.is_ascii()) {
            return Err(ContentTypeError::NonAscii { ch: bad });
        }

        let essence = value.split(';').next().unwrap_or(value).trim();
        let Some((ty, subtype)) = essence.split_once('/') else {
            return Err(ContentTypeError::MissingSlash);
        };
        validate_token(ty, "type")?;
        validate_token(subtype, "subtype")?;

        for param in value.split(';').skip(1) {
            let param = param.trim();
            if param.is_empty() {
                return Err(ContentTypeError::EmptyToken { part: "parameter" });
            }
            // `name=value` is validated loosely: a parameter without `=` (e.g.
            // a stray flag) is still a legal extension parameter.
            let name = param.split('=').next().unwrap_or(param).trim();
            validate_token(name, "parameter")?;
        }

        Ok(Self(value.to_owned()))
    }

    /// Infer from a file extension, degrading to
    /// [`DEFAULT_CONTENT_TYPE`] for anything unknown.
    pub fn from_extension(extension: &str) -> Self {
        let extension = extension.trim_start_matches('.').to_ascii_lowercase();
        let known = match extension.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "svg" => "image/svg+xml",
            "avif" => "image/avif",
            "pdf" => "application/pdf",
            "json" => "application/json",
            "ndjson" => "application/x-ndjson",
            "txt" | "log" => "text/plain",
            "md" => "text/markdown",
            "csv" => "text/csv",
            "html" | "htm" => "text/html",
            "css" => "text/css",
            "js" | "mjs" => "text/javascript",
            "xml" => "application/xml",
            "zip" => "application/zip",
            "gz" => "application/gzip",
            "zst" => "application/zstd",
            "tar" => "application/x-tar",
            "mp4" => "video/mp4",
            "webm" => "video/webm",
            "mov" => "video/quicktime",
            "mp3" => "audio/mpeg",
            "wav" => "audio/wav",
            "ogg" => "audio/ogg",
            "wasm" => "application/wasm",
            _ => DEFAULT_CONTENT_TYPE,
        };
        Self(known.to_owned())
    }

    /// Infer from a path's extension, degrading to
    /// [`DEFAULT_CONTENT_TYPE`] when there is none.
    pub fn infer_from_path(path: &Path) -> Self {
        match path.extension().and_then(|e| e.to_str()) {
            Some(ext) => Self::from_extension(ext),
            None => Self::default(),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The `type/subtype` part, without parameters.
    pub fn essence(&self) -> &str {
        self.0.split(';').next().unwrap_or(&self.0).trim()
    }

    pub fn is_default(&self) -> bool {
        self.0 == DEFAULT_CONTENT_TYPE
    }
}

impl Default for ContentType {
    fn default() -> Self {
        Self(DEFAULT_CONTENT_TYPE.to_owned())
    }
}

impl fmt::Display for ContentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn validate_token(token: &str, part: &'static str) -> Result<(), ContentTypeError> {
    if token.is_empty() {
        return Err(ContentTypeError::EmptyToken { part });
    }
    for ch in token.chars() {
        let ok = ch.is_ascii_alphanumeric()
            || matches!(
                ch,
                '!' | '#'
                    | '$'
                    | '%'
                    | '&'
                    | '\''
                    | '*'
                    | '+'
                    | '-'
                    | '.'
                    | '^'
                    | '_'
                    | '`'
                    | '|'
                    | '~'
            );
        if !ok {
            return Err(ContentTypeError::InvalidToken {
                token: token.to_owned(),
                ch,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Table of `(input, accepted)` for [`AssetKey::parse`].
    const KEY_TABLE: &[(&str, bool)] = &[
        // --- accept ---
        ("a", true),
        ("uploads/report.pdf", true),
        ("uploads/2026/09/report.pdf", true),
        ("uploads/.hidden", true),
        ("uploads/name.with.dots.tar.gz", true),
        ("uploads/a~b_c-d.e", true),
        ("uploads/0000", true),
        ("s3://bucket/uploads/a.png", true),
        ("gs://bucket/uploads/a.png", true),
        ("s3://bucket/deep/path/x", true),
        // --- reject: traversal / separators ---
        ("uploads/../a", false),
        ("..", false),
        ("uploads/./a", false),
        (".", false),
        ("uploads//a", false),
        ("//a", false),
        ("/uploads/a", false),
        ("uploads/a/", false),
        ("", false),
        // --- reject: reserved namespace ---
        ("_meta/a.visibility.json", false),
        ("_meta", false),
        ("_meta/x", false),
        // --- reject: scheme traps ---
        ("s3://bucket", false),
        ("gs://bucket", false),
        ("s3://bucket/", false),
        ("file://bucket/a", false),
        // --- reject: length ---
        // 1024 bytes total: 4 segments of 255 + 3 separators + 1 filler = 1024.
        ("aaaa/bbbb/cccc/dddd", true),
        // --- reject: alphabet ---
        ("uploads/a b", false),
        ("uploads/a+b", false),
        ("uploads/a%b", false),
        ("uploads/ünicode", false),
        ("uploads/emoji-🎉", false),
        ("uploads/back\\slash", false),
        ("uploads/a:b", false),
        ("uploads/a*b", false),
        ("uploads/a@b", false),
        ("uploads/a=b", false),
        ("uploads/a,b", false),
        ("uploads/a(b)", false),
        ("uploads/a'b", false),
        ("uploads/a!b", false),
        ("uploads/a#b", false),
        ("uploads/a$b", false),
        ("uploads/a&b", false),
        ("uploads/a;b", false),
    ];

    #[test]
    fn key_accept_reject_table() {
        for (input, accepted) in KEY_TABLE {
            let result = AssetKey::parse(input);
            assert_eq!(
                result.is_ok(),
                *accepted,
                "AssetKey::parse({input:?}) => {result:?}, expected accepted={accepted}"
            );
        }
    }

    #[test]
    fn key_length_bounds_are_enforced_exactly() {
        let segment = "a".repeat(MAX_SEGMENT_BYTES);
        assert!(AssetKey::parse(&segment).is_ok());
        assert!(matches!(
            AssetKey::parse(&"a".repeat(MAX_SEGMENT_BYTES + 1)),
            Err(KeyError::SegmentTooLong { .. })
        ));

        // 5 segments of 204 bytes + 4 separators = exactly MAX_KEY_BYTES.
        let part = "b".repeat(204);
        let at_limit = format!("{part}/{part}/{part}/{part}/{part}");
        assert_eq!(at_limit.len(), MAX_KEY_BYTES);
        assert!(AssetKey::parse(&at_limit).is_ok(), "{at_limit}");

        // One byte over: the total check fires before the per-segment check.
        let over_limit = format!("{at_limit}c");
        assert_eq!(over_limit.len(), MAX_KEY_BYTES + 1);
        assert!(matches!(
            AssetKey::parse(&over_limit),
            Err(KeyError::TooLong { .. })
        ));
    }

    #[test]
    fn rejection_reasons_are_specific() {
        assert_eq!(
            AssetKey::parse("a/../b"),
            Err(KeyError::DotDotSegment { index: 1 })
        );
        assert_eq!(
            AssetKey::parse("a/./b"),
            Err(KeyError::DotSegment { index: 1 })
        );
        assert_eq!(
            AssetKey::parse("a//b"),
            Err(KeyError::EmptySegment { index: 1 })
        );
        assert_eq!(
            AssetKey::parse("/a"),
            Err(KeyError::EmptySegment { index: 0 })
        );
        assert_eq!(
            AssetKey::parse("a/"),
            Err(KeyError::EmptySegment { index: 1 })
        );
        assert_eq!(
            AssetKey::parse("_meta/a"),
            Err(KeyError::ReservedMetaSegment)
        );
        assert_eq!(AssetKey::parse(""), Err(KeyError::Empty));
        assert!(matches!(
            AssetKey::parse("a/ü"),
            Err(KeyError::NonAscii { .. })
        ));
        assert!(matches!(
            AssetKey::parse("a b"),
            Err(KeyError::InvalidChar { ch: ' ', .. })
        ));
        assert!(matches!(
            AssetKey::parse(&"a".repeat(MAX_SEGMENT_BYTES + 1)),
            Err(KeyError::SegmentTooLong { .. })
        ));
        assert!(matches!(
            AssetKey::parse("s3://bucket"),
            Err(KeyError::MissingObjectPath { .. })
        ));
    }

    #[test]
    fn parse_physical_admits_the_sidecar_namespace() {
        let key = AssetKey::parse_physical("_meta/uploads/a.png.visibility.json").unwrap();
        assert!(key.is_physical_meta());
        assert_eq!(key.file_name(), "a.png.visibility.json");
        assert_eq!(key.segments().count(), 3);
        assert!(AssetKey::parse_physical("_meta").is_ok());
        assert!(AssetKey::parse_physical("a/b").is_ok());
    }

    #[test]
    fn scheme_qualified_keys_strip_the_bucket() {
        assert_eq!(
            AssetKey::parse("s3://bucket/uploads/a.png")
                .unwrap()
                .as_str(),
            "uploads/a.png"
        );
        assert_eq!(
            AssetKey::parse("gs://bucket/uploads/a.png")
                .unwrap()
                .as_str(),
            "uploads/a.png"
        );
    }

    #[test]
    fn key_helpers_are_consistent() {
        let key = AssetKey::parse("uploads/2026/report.tar.gz").unwrap();
        assert_eq!(key.file_name(), "report.tar.gz");
        assert_eq!(key.extension().as_deref(), Some("gz"));
        assert_eq!(key.parent().unwrap().as_str(), "uploads/2026/");
        assert_eq!(
            key.segments().collect::<Vec<_>>(),
            ["uploads", "2026", "report.tar.gz"]
        );
        assert!(key.starts_with(&AssetPrefix::parse("uploads/").unwrap()));
        assert!(!key.starts_with(&AssetPrefix::parse("other/").unwrap()));
        assert_eq!(
            key.strip_prefix(&AssetPrefix::parse("uploads/").unwrap()),
            Some("2026/report.tar.gz")
        );
        assert_eq!(AssetKey::parse("a").unwrap().parent(), None);
        assert_eq!(AssetKey::parse("archive").unwrap().extension(), None);
    }

    #[test]
    fn key_with_prefix_joins_and_validates() {
        let prefix = AssetPrefix::parse("uploads/").unwrap();
        assert_eq!(
            AssetKey::with_prefix(&prefix, "a/b.png").unwrap().as_str(),
            "uploads/a/b.png"
        );
        assert_eq!(
            AssetKey::with_prefix(&AssetPrefix::default(), "a.png")
                .unwrap()
                .as_str(),
            "a.png"
        );
        assert!(AssetKey::with_prefix(&prefix, "../a.png").is_err());
    }

    /// Table of `(input, accepted)` for [`AssetPrefix::parse`].
    const PREFIX_TABLE: &[(&str, bool)] = &[
        ("", true),
        ("uploads/", true),
        ("uploads", true),
        ("a/b/", true),
        ("a/b", true),
        ("_meta/", true),
        ("_meta", true),
        ("/uploads/", false),
        ("a//b", false),
        ("a/../b", false),
        ("a/./b", false),
        ("a b/", false),
        ("ü/", false),
    ];

    #[test]
    fn prefix_accept_reject_table() {
        for (input, accepted) in PREFIX_TABLE {
            let result = AssetPrefix::parse(input);
            assert_eq!(
                result.is_ok(),
                *accepted,
                "AssetPrefix::parse({input:?}) => {result:?}, expected accepted={accepted}"
            );
        }
    }

    #[test]
    fn prefix_normalizes_to_a_single_trailing_slash() {
        assert_eq!(AssetPrefix::parse("").unwrap().normalized(), "");
        assert_eq!(
            AssetPrefix::parse("uploads/").unwrap().normalized(),
            "uploads/"
        );
        assert_eq!(
            AssetPrefix::parse("uploads").unwrap().normalized(),
            "uploads/"
        );
        assert!(AssetPrefix::parse("").unwrap().is_empty());
        assert!(!AssetPrefix::parse("uploads/").unwrap().is_empty());
    }

    /// Table of `(input, accepted)` for [`ContentType::parse`].
    const CONTENT_TYPE_TABLE: &[(&str, bool)] = &[
        ("application/octet-stream", true),
        ("image/png", true),
        ("text/plain; charset=utf-8", true),
        ("text/plain;charset=utf-8", true),
        ("application/vnd.api+json", true),
        ("application/x-www-form-urlencoded", true),
        ("multipart/form-data; boundary=----x", true),
        ("text/plain;", false),
        ("plain", false),
        ("/plain", false),
        ("text/", false),
        ("", false),
        ("   ", false),
        ("text /plain", false),
        ("text/plain ü", false),
        ("text/plain; charset", true),
        // Loose parameter form: `name = value` still names a valid token.
        ("text/plain; charset = utf-8", true),
    ];

    #[test]
    fn content_type_accept_reject_table() {
        for (input, accepted) in CONTENT_TYPE_TABLE {
            let result = ContentType::parse(input);
            assert_eq!(
                result.is_ok(),
                *accepted,
                "ContentType::parse({input:?}) => {result:?}, expected accepted={accepted}"
            );
        }
    }

    #[test]
    fn content_type_length_bound_is_enforced() {
        let subtype = "a".repeat(MAX_CONTENT_TYPE_BYTES - "text/".len());
        let ok = format!("text/{subtype}");
        assert_eq!(ok.len(), MAX_CONTENT_TYPE_BYTES);
        assert!(ContentType::parse(&ok).is_ok());
        assert!(ContentType::parse(&format!("{ok}a")).is_err());
    }

    #[test]
    fn content_type_inference_degrades_to_octet_stream() {
        assert_eq!(ContentType::from_extension("PNG").as_str(), "image/png");
        assert_eq!(
            ContentType::from_extension(".pdf").as_str(),
            "application/pdf"
        );
        assert_eq!(
            ContentType::from_extension("nope").as_str(),
            DEFAULT_CONTENT_TYPE
        );
        assert!(ContentType::from_extension("nope").is_default());
        assert!(ContentType::default().is_default());
        assert_eq!(
            ContentType::infer_from_path(Path::new("/tmp/a.png")).as_str(),
            "image/png"
        );
        assert_eq!(
            ContentType::infer_from_path(Path::new("/tmp/a")).as_str(),
            DEFAULT_CONTENT_TYPE
        );
    }

    #[test]
    fn content_type_essence_drops_parameters() {
        let ct = ContentType::parse("text/plain; charset=utf-8").unwrap();
        assert_eq!(ct.essence(), "text/plain");
        assert_eq!(ct.as_str(), "text/plain; charset=utf-8");
    }
}
