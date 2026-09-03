//! Durable, session-scoped descriptions and transcripts for media assets.
//!
//! The store contains only scrubbed text and confined asset metadata. Raw
//! media bytes and provider request payloads never enter this file.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const MEDIA_DESCRIPTORS_FILE: &str = "media_descriptors.jsonl";
pub const MEDIA_DESCRIPTOR_SCHEMA_VERSION: u32 = 1;
pub const MAX_MEDIA_DESCRIPTOR_ENTRIES: usize = 4_096;
pub const MAX_MEDIA_DESCRIPTOR_TEXT_BYTES: usize = 256 * 1024;
pub const MAX_MEDIA_DESCRIPTOR_LINE_BYTES: usize = MAX_MEDIA_DESCRIPTOR_TEXT_BYTES + 16 * 1024;
/// Aggregate scan/write cap for one session descriptor store.
pub const MAX_MEDIA_DESCRIPTOR_FILE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaModality {
    Image,
    Audio,
    Video,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaDescriptorSource {
    UserAttachment,
    ToolRead,
    CompactionBackfill,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MediaDescriptorKey {
    pub modality: MediaModality,
    pub content_fingerprint: String,
    pub source: MediaDescriptorSource,
    pub prompt_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaDescriptor {
    pub schema_version: u32,
    pub key: MediaDescriptorKey,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_path: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl MediaDescriptor {
    pub fn new(
        key: MediaDescriptorKey,
        description: String,
        mime_type: Option<String>,
        model_id: Option<String>,
        provider: Option<String>,
        asset_path: Option<String>,
    ) -> io::Result<Self> {
        validate_descriptor_key(&key)?;
        validate_descriptor_text(&description)?;
        if let Some(path) = asset_path.as_deref() {
            validate_asset_path(path)?;
        }
        Ok(Self {
            schema_version: MEDIA_DESCRIPTOR_SCHEMA_VERSION,
            key,
            mime_type,
            model_id,
            provider,
            description,
            asset_path,
            created_at: chrono::Utc::now(),
        })
    }

    fn validate(&self) -> io::Result<()> {
        if self.schema_version != MEDIA_DESCRIPTOR_SCHEMA_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "unsupported media descriptor schema {}",
                    self.schema_version
                ),
            ));
        }
        validate_descriptor_key(&self.key)?;
        validate_descriptor_text(&self.description)?;
        if let Some(path) = self.asset_path.as_deref() {
            validate_asset_path(path)?;
        }
        Ok(())
    }
}

pub type MediaDescriptorMap = HashMap<MediaDescriptorKey, MediaDescriptor>;

#[derive(Debug)]
pub struct MediaDescriptorStore {
    path: PathBuf,
    /// Lock-free shared map. Readers (`get`, `snapshot`) load without blocking
    /// and `snapshot` is O(1) (an `Arc` clone of the current generation).
    /// Writers clone-on-write and publish through `rcu`, whose compare-and-
    /// swap retry keeps racing inserts from dropping each other's entries.
    entries: arc_swap::ArcSwap<MediaDescriptorMap>,
}

impl MediaDescriptorStore {
    pub fn load(session_dir: &Path) -> io::Result<Self> {
        let path = session_dir.join(MEDIA_DESCRIPTORS_FILE);
        let entries = load_descriptor_map(&path)?;
        Ok(Self {
            path,
            entries: arc_swap::ArcSwap::from_pointee(entries),
        })
    }

    pub fn empty(session_dir: &Path) -> Self {
        Self {
            path: session_dir.join(MEDIA_DESCRIPTORS_FILE),
            entries: arc_swap::ArcSwap::from_pointee(HashMap::new()),
        }
    }

    pub fn get(&self, key: &MediaDescriptorKey) -> Option<MediaDescriptor> {
        self.entries.load().get(key).cloned()
    }

    pub fn snapshot(&self) -> Arc<MediaDescriptorMap> {
        self.entries.load_full()
    }

    pub fn insert(&self, descriptor: MediaDescriptor) -> io::Result<()> {
        descriptor.validate()?;
        // Limit pre-check against the lock-free snapshot (matching the
        // serialized original: replacing an existing key is always allowed).
        // A same-instant race of distinct keys can overshoot the limit by one;
        // the file-size cap below still bounds the on-disk footprint.
        let snapshot = self.entries.load();
        let replacing = snapshot.contains_key(&descriptor.key);
        if !replacing && snapshot.len() >= MAX_MEDIA_DESCRIPTOR_ENTRIES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "media descriptor store reached its entry limit",
            ));
        }
        drop(snapshot);
        let record_len = descriptor_record(&descriptor)?.len() as u64;
        let needs_compaction = replacing
            || std::fs::metadata(&self.path)
                .map(|metadata| {
                    metadata.len().saturating_add(record_len) > MAX_MEDIA_DESCRIPTOR_FILE_BYTES
                })
                .unwrap_or(false);
        if needs_compaction {
            // Rewrite path: clone-on-write from the lock-free snapshot,
            // atomically replace the file with the full map (tempfile +
            // rename), then publish the same map. The rewrite runs inside the
            // rcu closure so every published generation is fully persisted;
            // a racing insert triggers a retry that re-persists the merged
            // generation. On rewrite failure the previous generation is
            // published unchanged and the error is surfaced.
            let rewrite_error = std::cell::Cell::new(None);
            self.entries.rcu(|current| {
                // `current` is `&Arc<MediaDescriptorMap>`: clone the inner map,
                // not the Arc, so the published generation is a new owned map.
                let mut next = MediaDescriptorMap::clone(current);
                next.insert(descriptor.key.clone(), descriptor.clone());
                match rewrite_descriptors(&self.path, &next) {
                    Ok(()) => {
                        rewrite_error.set(None);
                        next
                    }
                    Err(error) => {
                        rewrite_error.set(Some(error));
                        MediaDescriptorMap::clone(current)
                    }
                }
            });
            if let Some(error) = rewrite_error.into_inner() {
                return Err(error);
            }
            return Ok(());
        }
        // Append path: a single append write is line-atomic across concurrent
        // writers, and the rcu publish retries from the winner generation so
        // concurrent inserts never lose each other's in-memory entries.
        append_descriptor(&self.path, &descriptor)?;
        self.entries.rcu(|current| {
            let mut next = MediaDescriptorMap::clone(current);
            next.insert(descriptor.key.clone(), descriptor.clone());
            next
        });
        Ok(())
    }
}

fn validate_descriptor_key(key: &MediaDescriptorKey) -> io::Result<()> {
    for (label, value) in [
        ("content fingerprint", key.content_fingerprint.as_str()),
        ("prompt fingerprint", key.prompt_fingerprint.as_str()),
    ] {
        if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid media descriptor {label}"),
            ));
        }
    }
    Ok(())
}

fn validate_descriptor_text(description: &str) -> io::Result<()> {
    if description.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "media descriptor text is empty",
        ));
    }
    if description.len() > MAX_MEDIA_DESCRIPTOR_TEXT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "media descriptor text exceeds its byte limit",
        ));
    }
    Ok(())
}

fn validate_asset_path(path: &str) -> io::Result<()> {
    let path = Path::new(path);
    if path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "media descriptor asset path must be session-relative and confined",
        ));
    }
    Ok(())
}

fn load_descriptor_map(path: &Path) -> io::Result<MediaDescriptorMap> {
    let file = match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(error) => return Err(error),
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = std::fs::Permissions::from_mode(0o600);
        file.set_permissions(permissions)?;
    }
    let mut reader = BufReader::new(file);
    let mut entries = HashMap::new();
    let mut scanned_bytes = 0u64;
    while scanned_bytes < MAX_MEDIA_DESCRIPTOR_FILE_BYTES {
        let mut line = Vec::new();
        let remaining = MAX_MEDIA_DESCRIPTOR_FILE_BYTES - scanned_bytes;
        let line_budget = remaining.min((MAX_MEDIA_DESCRIPTOR_LINE_BYTES + 1) as u64);
        let bytes_read = reader
            .by_ref()
            .take(line_budget)
            .read_until(b'\n', &mut line)?;
        if bytes_read == 0 {
            break;
        }
        scanned_bytes = scanned_bytes.saturating_add(bytes_read as u64);
        if line.len() > MAX_MEDIA_DESCRIPTOR_LINE_BYTES {
            // Avoid allocating an attacker-controlled line. If the bounded
            // read stopped before its newline, discard the rest in-place.
            if line.last() != Some(&b'\n') {
                let remaining = MAX_MEDIA_DESCRIPTOR_FILE_BYTES.saturating_sub(scanned_bytes);
                scanned_bytes =
                    scanned_bytes.saturating_add(discard_until_newline(&mut reader, remaining)?);
            }
            continue;
        }
        while matches!(line.last(), Some(b'\n' | b'\r')) {
            line.pop();
        }
        if line.is_empty() {
            continue;
        }
        let Ok(descriptor) = serde_json::from_slice::<MediaDescriptor>(&line) else {
            // A partially written trailing record or a newer malformed record
            // must not make the rest of the session unreadable.
            continue;
        };
        if descriptor.validate().is_ok() {
            if !entries.contains_key(&descriptor.key)
                && entries.len() >= MAX_MEDIA_DESCRIPTOR_ENTRIES
            {
                continue;
            }
            entries.insert(descriptor.key.clone(), descriptor);
        }
    }
    Ok(entries)
}

fn discard_until_newline(reader: &mut impl BufRead, mut budget: u64) -> io::Result<u64> {
    let mut discarded = 0u64;
    while budget > 0 {
        let buffer = reader.fill_buf()?;
        if buffer.is_empty() {
            break;
        }
        let available = buffer.len().min(budget as usize);
        let consumed = buffer[..available]
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available, |index| index + 1);
        let found_newline = buffer.get(consumed - 1) == Some(&b'\n');
        reader.consume(consumed);
        discarded = discarded.saturating_add(consumed as u64);
        budget = budget.saturating_sub(consumed as u64);
        if found_newline {
            break;
        }
    }
    Ok(discarded)
}

fn descriptor_record(descriptor: &MediaDescriptor) -> io::Result<Vec<u8>> {
    let mut record = serde_json::to_vec(descriptor)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    record.push(b'\n');
    Ok(record)
}

fn rewrite_descriptors(path: &Path, entries: &MediaDescriptorMap) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "descriptor path has no parent")
    })?;
    std::fs::create_dir_all(parent)?;
    let mut descriptors: Vec<_> = entries.values().collect();
    descriptors.sort_by(|left, right| {
        left.created_at.cmp(&right.created_at).then_with(|| {
            left.key
                .content_fingerprint
                .cmp(&right.key.content_fingerprint)
        })
    });

    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tmp.as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    let mut written = 0u64;
    for descriptor in descriptors {
        let record = descriptor_record(descriptor)?;
        written = written.saturating_add(record.len() as u64);
        if written > MAX_MEDIA_DESCRIPTOR_FILE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "media descriptor store exceeds its file-size limit",
            ));
        }
        tmp.write_all(&record)?;
    }
    tmp.flush()?;
    tmp.as_file().sync_data()?;
    tmp.persist(path).map_err(|error| error.error)?;
    Ok(())
}

fn append_descriptor(path: &Path, descriptor: &MediaDescriptor) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "descriptor path has no parent")
    })?;
    std::fs::create_dir_all(parent)?;
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    let record = descriptor_record(descriptor)?;
    // One append write prevents concurrent writers from interleaving a JSON
    // body and its terminator. A torn final record remains recoverable because
    // the loader ignores malformed lines.
    file.write_all(&record)?;
    file.flush()?;
    file.sync_data()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor() -> MediaDescriptor {
        MediaDescriptor::new(
            MediaDescriptorKey {
                modality: MediaModality::Image,
                content_fingerprint: "a".repeat(64),
                source: MediaDescriptorSource::ToolRead,
                prompt_fingerprint: "b".repeat(64),
            },
            "A small red square.".to_owned(),
            Some("image/png".to_owned()),
            Some("grok-4.5".to_owned()),
            Some("xai".to_owned()),
            Some("assets/example.png".to_owned()),
        )
        .unwrap()
    }

    #[test]
    fn descriptor_store_round_trips_and_deduplicates_by_key() {
        let dir = tempfile::tempdir().unwrap();
        let store = MediaDescriptorStore::load(dir.path()).unwrap();
        let first = descriptor();
        store.insert(first.clone()).unwrap();
        let mut replacement = first.clone();
        replacement.description = "Replacement description.".to_owned();
        store.insert(replacement.clone()).unwrap();

        let reloaded = MediaDescriptorStore::load(dir.path()).unwrap();
        assert_eq!(reloaded.snapshot().len(), 1);
        assert_eq!(
            reloaded.get(&first.key).unwrap().description,
            replacement.description
        );
    }

    #[test]
    fn descriptor_replacement_rewrites_instead_of_growing_jsonl() {
        let dir = tempfile::tempdir().unwrap();
        let store = MediaDescriptorStore::load(dir.path()).unwrap();
        let first = descriptor();
        store.insert(first.clone()).unwrap();
        let path = dir.path().join(MEDIA_DESCRIPTORS_FILE);
        let first_len = std::fs::metadata(&path).unwrap().len();

        let mut replacement = first;
        replacement.description = "Short replacement.".to_owned();
        store.insert(replacement).unwrap();
        let replacement_len = std::fs::metadata(&path).unwrap().len();

        assert!(replacement_len < first_len.saturating_mul(2));
        assert_eq!(std::fs::read_to_string(path).unwrap().lines().count(), 1);
    }

    #[test]
    fn descriptor_store_ignores_partial_trailing_record() {
        let dir = tempfile::tempdir().unwrap();
        let store = MediaDescriptorStore::load(dir.path()).unwrap();
        let entry = descriptor();
        store.insert(entry.clone()).unwrap();
        std::fs::OpenOptions::new()
            .append(true)
            .open(dir.path().join(MEDIA_DESCRIPTORS_FILE))
            .unwrap()
            .write_all(b"{\"schema_version\":")
            .unwrap();

        let reloaded = MediaDescriptorStore::load(dir.path()).unwrap();
        assert!(reloaded.get(&entry.key).is_some());
    }

    #[test]
    fn descriptor_store_skips_oversized_line_without_allocating_it_whole() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(MEDIA_DESCRIPTORS_FILE);
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(&vec![b'x'; MAX_MEDIA_DESCRIPTOR_LINE_BYTES + 128])
            .unwrap();
        file.write_all(b"\n").unwrap();
        serde_json::to_writer(&mut file, &descriptor()).unwrap();
        file.write_all(b"\n").unwrap();

        let store = MediaDescriptorStore::load(dir.path()).unwrap();
        assert_eq!(store.snapshot().len(), 1);
    }

    #[test]
    fn malformed_lines_do_not_consume_valid_entry_budget() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(MEDIA_DESCRIPTORS_FILE);
        let mut file = std::fs::File::create(&path).unwrap();
        for _ in 0..=MAX_MEDIA_DESCRIPTOR_ENTRIES {
            file.write_all(b"not-json\n").unwrap();
        }
        serde_json::to_writer(&mut file, &descriptor()).unwrap();
        file.write_all(b"\n").unwrap();

        let store = MediaDescriptorStore::load(dir.path()).unwrap();
        assert_eq!(store.snapshot().len(), 1);
    }

    #[test]
    fn descriptor_rejects_escaping_asset_path() {
        let mut entry = descriptor();
        entry.asset_path = Some("../outside.png".to_owned());
        assert!(entry.validate().is_err());
    }

    #[test]
    fn descriptor_file_is_owner_only_on_unix() {
        let dir = tempfile::tempdir().unwrap();
        let store = MediaDescriptorStore::load(dir.path()).unwrap();
        store.insert(descriptor()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.path().join(MEDIA_DESCRIPTORS_FILE))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }
    }
}
