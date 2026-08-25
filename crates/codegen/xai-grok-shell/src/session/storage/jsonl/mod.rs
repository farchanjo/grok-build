use super::{PersistedData, SessionUpdateEnvelope, StorageAdapter};
use crate::inference::types::ChatRequestMessage;
use crate::inference::{
    ContentPart, ConversationItem, conversation_truncate_for_prompt, transform_conversation_cwd,
};
use crate::session::info::Info;
use crate::session::persistence::{CHAT_FORMAT_VERSION, Summary};
use crate::tools::todo::TodoState;
use agent_client_protocol as acp;
use async_trait::async_trait;
use fs2::FileExt;
use std::fs::OpenOptions;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Seek, Write};
use std::ops::ControlFlow;
use std::path::{Component, Path, PathBuf};
use xai_chat_state::StrictAppendAck;
use xai_grok_workspace::session::file_state::RewindPoint;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct PendingCompactionFile {
    checkpoint_id: String,
    previous_history: Vec<ConversationItem>,
    /// Identity of the replacement base written before the marker. Startup uses
    /// it to preserve items appended while marker durability was indeterminate
    /// without retaining a second full conversation buffer.
    compacted_history_len: usize,
    compacted_history_fingerprint: [u8; 32],
}

#[derive(Clone)]
enum SessionDirMode {
    FromRoot(PathBuf),
    Explicit(PathBuf),
}
#[derive(Clone, Copy)]
pub(crate) enum AppendDurability {
    Buffered,
    Durable,
}
/// JSONL storage under `{root}/sessions/{url_encoded_cwd}/{session_id}/`.
#[derive(Clone)]
pub struct JsonlStorageAdapter {
    dir_mode: SessionDirMode,
    #[cfg(test)]
    update_append_probe: Option<std::sync::Arc<AppendProbe>>,
}
#[cfg(test)]
type AppendProbe = dyn Fn(AppendDurability) -> io::Result<()> + Send + Sync;
impl Default for JsonlStorageAdapter {
    fn default() -> Self {
        Self::new()
    }
}
impl JsonlStorageAdapter {
    pub fn new() -> Self {
        Self {
            dir_mode: SessionDirMode::FromRoot(crate::util::grok_home::grok_home()),
            #[cfg(test)]
            update_append_probe: None,
        }
    }
    pub fn with_root(root_dir: PathBuf) -> Self {
        Self {
            dir_mode: SessionDirMode::FromRoot(root_dir),
            #[cfg(test)]
            update_append_probe: None,
        }
    }
    /// Create an adapter that writes directly to `session_dir`, bypassing
    /// the `{root}/sessions/{cwd}/{id}/` path computation.
    ///
    /// Used for subagent child sessions whose files live under the parent's
    /// session directory: `{parent_session_dir}/subagents/{subagent_id}/`.
    pub fn with_explicit_session_dir(session_dir: PathBuf) -> Self {
        Self {
            dir_mode: SessionDirMode::Explicit(session_dir),
            #[cfg(test)]
            update_append_probe: None,
        }
    }
    #[cfg(test)]
    pub(crate) fn with_update_append_probe(
        session_dir: PathBuf,
        append_probe: impl Fn(AppendDurability) -> io::Result<()> + Send + Sync + 'static,
    ) -> Self {
        Self {
            dir_mode: SessionDirMode::Explicit(session_dir),
            update_append_probe: Some(std::sync::Arc::new(append_probe)),
        }
    }
    /// Load chat history from a specific directory.
    /// Used by fork bootstrap to load the copied parent conversation.
    pub fn load_chat_history_from_dir(
        &self,
        dir: &std::path::Path,
    ) -> std::io::Result<Vec<ConversationItem>> {
        let chat_file = dir.join(super::CHAT_HISTORY_FILE);
        self.read_chat_history_sync(chat_file, CHAT_FORMAT_VERSION)
    }
    fn session_dir(&self, info: &Info) -> PathBuf {
        match &self.dir_mode {
            SessionDirMode::FromRoot(root) => {
                crate::util::grok_home::sessions_cwd_dir_in(root, &info.cwd)
                    .join(info.id.to_string())
            }
            SessionDirMode::Explicit(dir) => dir.clone(),
        }
    }
    fn confined_session_dir(&self, info: &Info) -> io::Result<PathBuf> {
        validate_session_id_component(info.id.0.as_ref())?;
        let dir = self.session_dir(info);
        if let SessionDirMode::FromRoot(root) = &self.dir_mode {
            let session_root = crate::util::grok_home::sessions_cwd_dir_in(root, &info.cwd);
            if dir.parent() != Some(session_root.as_path())
                || !dir.starts_with(root.join("sessions"))
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "session path escaped its intended root",
                ));
            }
        }
        Ok(dir)
    }
    /// Create `info`'s session dir owner-only. `FromRoot` also ensures the
    /// `<encoded-cwd>` shield + root; `Explicit` parents are caller-owned.
    fn create_session_dir_owner_only(&self, info: &Info) -> io::Result<PathBuf> {
        let dir = self.session_dir(info);
        if let SessionDirMode::FromRoot(root) = &self.dir_mode {
            let _ = crate::util::grok_home::ensure_sessions_cwd_dir_in(root, &info.cwd);
        }
        crate::util::grok_home::create_dir_all_owner_only(&dir)?;
        Ok(dir)
    }
    pub(super) fn updates_file(&self, info: &Info) -> PathBuf {
        self.session_dir(info).join(super::UPDATES_FILE)
    }
    fn chat_file(&self, info: &Info) -> PathBuf {
        self.session_dir(info).join(super::CHAT_HISTORY_FILE)
    }
    fn compaction_pending_file(&self, info: &Info) -> PathBuf {
        self.session_dir(info).join(super::COMPACTION_PENDING_FILE)
    }
    fn ensure_chat_history(&self, info: &Info, chat_format_version: u8) -> io::Result<()> {
        if chat_format_version != crate::session::persistence::CHAT_FORMAT_VERSION {
            return Ok(());
        }
        let chat_file = self.chat_file(info);
        let pending_file = self.compaction_pending_file(info);
        if pending_file.exists() {
            let bytes = std::fs::read(&pending_file)?;
            let pending: PendingCompactionFile = serde_json::from_slice(&bytes)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            let committed_marker = self
                .read_updates_jsonl(self.updates_file(info))?
                .into_iter()
                .find_map(|update| {
                    let super::SessionUpdate::Xai(notification) = update else {
                        return None;
                    };
                    let crate::extensions::notification::SessionUpdate::CompactionCheckpoint(
                        marker,
                    ) = notification.update
                    else {
                        return None;
                    };
                    (marker.checkpoint_id == pending.checkpoint_id).then_some(*marker)
                });
            if let Some(marker) = committed_marker {
                super::load_validated_compaction_checkpoint(&self.session_dir(info), &marker)?;
                tracing::warn!(
                    checkpoint_id = %pending.checkpoint_id,
                    "clearing pending compaction recovery marker after committed checkpoint"
                );
            } else {
                let current =
                    self.read_chat_history_sync(chat_file.clone(), chat_format_version)?;
                let starts_with_fingerprint = |base_len: usize,
                                               base_fingerprint: [u8; 32]|
                 -> io::Result<bool> {
                    if current.len() < base_len {
                        return Ok(false);
                    }
                    Ok(
                        xai_chat_state::fingerprint_conversation_items(&current[..base_len])
                            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
                            == base_fingerprint,
                    )
                };
                let previous_len = pending.previous_history.len();
                let previous_fingerprint =
                    xai_chat_state::fingerprint_conversation_items(&pending.previous_history)
                        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
                let restored = if starts_with_fingerprint(
                    pending.compacted_history_len,
                    pending.compacted_history_fingerprint,
                )? {
                    let appended_tail = &current[pending.compacted_history_len..];
                    let mut restored = pending.previous_history;
                    restored.extend_from_slice(appended_tail);
                    tracing::warn!(
                        checkpoint_id = %pending.checkpoint_id,
                        appended_items = appended_tail.len(),
                        "rolling back history left before compaction marker commit"
                    );
                    Some(restored)
                } else if starts_with_fingerprint(previous_len, previous_fingerprint)? {
                    tracing::warn!(
                        checkpoint_id = %pending.checkpoint_id,
                        appended_items = current.len().saturating_sub(previous_len),
                        "pending compaction history was already rolled back"
                    );
                    None
                } else {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "pending compaction history matches neither transaction base",
                    ));
                };
                if let Some(restored) = restored {
                    super::write_jsonl_atomic(&chat_file, &restored)?;
                }
            }
            match std::fs::remove_file(&pending_file) {
                Ok(()) => super::sync_parent_directory(&pending_file)?,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        } else if std::fs::metadata(&chat_file).map(|m| m.len()).unwrap_or(0) == 0 {
            super::chat_rebuild::rebuild_chat_history(&self.session_dir(info))?;
        }
        Ok(())
    }
    fn summary_file(&self, info: &Info) -> PathBuf {
        self.session_dir(info).join(super::SUMMARY_FILE)
    }
    fn summary_lock_file(&self, info: &Info) -> PathBuf {
        self.session_dir(info)
            .join(format!("{}.lock", super::SUMMARY_FILE))
    }
    fn plan_file(&self, info: &Info) -> PathBuf {
        self.session_dir(info).join(super::PLAN_FILE)
    }
    fn plan_mode_state_file(&self, info: &Info) -> PathBuf {
        self.session_dir(info).join(super::PLAN_MODE_FILE)
    }
    fn signals_file(&self, info: &Info) -> PathBuf {
        self.session_dir(info).join(super::SIGNALS_FILE)
    }
    fn announcement_state_file(&self, info: &Info) -> PathBuf {
        self.session_dir(info).join(super::ANNOUNCEMENT_STATE_FILE)
    }
    fn goal_mode_state_file(&self, info: &Info) -> PathBuf {
        self.session_dir(info).join(super::GOAL_STATE_FILE)
    }
    fn workflows_dir(&self, info: &Info) -> PathBuf {
        self.session_dir(info).join("workflows")
    }
    fn workflow_run_dir(&self, info: &Info, run_id: &str) -> io::Result<PathBuf> {
        crate::session::workflow::store::validate_run_id(run_id)?;
        Ok(self.workflows_dir(info).join(run_id))
    }
    fn workflow_run_state_file(&self, info: &Info, run_id: &str) -> io::Result<PathBuf> {
        Ok(self.workflow_run_dir(info, run_id)?.join("state.json"))
    }
    fn rewind_points_file(&self, info: &Info) -> PathBuf {
        self.session_dir(info).join("rewind_points.jsonl")
    }
    fn feedback_file(&self, info: &Info) -> PathBuf {
        self.session_dir(info).join("feedback.jsonl")
    }
    fn btw_history_file(&self, info: &Info) -> PathBuf {
        self.session_dir(info).join("btw_history.jsonl")
    }
    /// Enumerate all session directories, optionally filtered by cwd.
    ///
    /// Returns the path to each session directory (not the summary file).
    /// Shared by both `list_sessions` (full scan) and `list_sessions_recent`
    /// (mtime-based tail).
    fn scan_session_dirs(&self, cwd: Option<&str>) -> io::Result<Vec<PathBuf>> {
        let root_dir = match &self.dir_mode {
            SessionDirMode::FromRoot(root) => root,
            SessionDirMode::Explicit(_) => return Ok(Vec::new()),
        };
        crate::session::storage::relocation::RelocationView::load(root_dir)
            .and_then(|view| view.session_dirs(cwd))
            .map_err(io::Error::other)
    }
    fn list_sessions_sync(&self, cwd: Option<&str>) -> io::Result<Vec<Summary>> {
        let session_dirs = self.scan_session_dirs(cwd)?;
        let mut summaries = Vec::new();
        for session_dir in session_dirs {
            // Contained read only. Intermediate `sessions` symlink / owner /
            // mode failures are skipped (picker must not path-adopt attacker
            // summaries). Corrupt JSON is also skipped.
            match super::model_route::read_summary_contained(&session_dir) {
                Ok(summary) if !summary.is_hidden() => summaries.push(summary),
                _ => continue,
            }
        }
        summaries.sort_by_cached_key(|s| {
            (
                std::cmp::Reverse(s.last_active_at.unwrap_or(s.updated_at)),
                s.info.id.0.to_string(),
            )
        });
        Ok(summaries)
    }
    /// List the N most recently active session summaries across all workspaces.
    ///
    /// Each candidate is loaded via the multi-component trusted-root walk.
    /// Entries whose walk fails (ELOOP on intermediate `sessions`, owner
    /// mismatch, missing summary) are **skipped** — never path-read.
    /// Order uses `last_active_at` else `updated_at` (not path mtime, which
    /// would follow a planted `sessions` symlink).
    pub async fn list_sessions_recent(&self, limit: usize) -> io::Result<Vec<Summary>> {
        let session_dirs = self.scan_session_dirs(None)?;
        let mut summaries = Vec::new();
        for session_dir in session_dirs {
            match super::model_route::read_summary_contained(&session_dir) {
                Ok(summary) if !summary.is_hidden() => summaries.push(summary),
                _ => continue,
            }
        }
        summaries.sort_by_cached_key(|s| {
            (
                std::cmp::Reverse(s.last_active_at.unwrap_or(s.updated_at)),
                s.info.id.0.to_string(),
            )
        });
        if summaries.len() > limit {
            summaries.truncate(limit);
        }
        Ok(summaries)
    }
    async fn append_jsonl<T: serde::Serialize>(&self, path: PathBuf, data: &T) -> io::Result<()> {
        self.append_jsonl_with_durability(path, data, AppendDurability::Buffered)
            .await
    }
    async fn append_jsonl_with_durability<T: serde::Serialize>(
        &self,
        path: PathBuf,
        data: &T,
        durability: AppendDurability,
    ) -> io::Result<()> {
        let mut line =
            serde_json::to_vec(data).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        line.push(b'\n');
        Self::append_jsonl_line_blocking(path, line, durability).await
    }
    async fn append_jsonl_line_blocking(
        path: PathBuf,
        line: Vec<u8>,
        durability: AppendDurability,
    ) -> io::Result<()> {
        tokio::task::spawn_blocking(move || Self::append_jsonl_line_sync(&path, line, durability))
            .await
            .map_err(io::Error::other)?
    }
    /// Append one JSONL record, healing a torn tail before writing.
    ///
    /// Appends are not crash-atomic: a process kill / `ENOSPC` mid-`write_all`
    /// (e.g. the auto-update leader relaunch aborting a persistence actor
    /// mid-append) leaves the file ending in a *partial* record with no
    /// trailing newline. Because append failures are logged-and-continued by
    /// the persistence actor, a plain `O_APPEND` write of the next record
    /// would concatenate it onto that partial line, producing a merged line
    /// that fails to parse (``expected `,` or `}` at line 1 column N``) and —
    /// before the readers became corruption-tolerant — bricked session resume.
    ///
    /// Before writing, check the last byte: if it isn't `\n`, prepend one so
    /// the torn record is terminated as its own (single) corrupt line. This
    /// bounds the damage of any torn write to exactly one record, which the
    /// lenient readers (e.g. [`Self::read_chat_history_sync`]) then skip.
    async fn sync_file_path_durable(path: PathBuf) -> io::Result<()> {
        tokio::task::spawn_blocking(move || {
            let file = OpenOptions::new().read(true).open(&path)?;
            Self::sync_file_durable(&file)
        })
        .await
        .map_err(io::Error::other)?
    }
    fn append_jsonl_line_sync(
        path: &Path,
        line: Vec<u8>,
        durability: AppendDurability,
    ) -> io::Result<()> {
        Self::append_jsonl_line_sync_with(path, line, durability, Self::sync_file_durable, || {
            Self::sync_parent_directory(path)
        })
    }
    fn append_durable_jsonl_line_commit_aware(
        path: &Path,
        line: Vec<u8>,
    ) -> Result<(), super::AppendUpdateError> {
        Self::append_durable_jsonl_line_commit_aware_with(
            path,
            line,
            Self::sync_file_durable,
            || Self::sync_parent_directory(path),
        )
    }
    fn append_durable_jsonl_line_commit_aware_with(
        path: &Path,
        mut line: Vec<u8>,
        mut sync_file: impl FnMut(&std::fs::File) -> io::Result<()>,
        mut sync_parent: impl FnMut() -> io::Result<()>,
    ) -> Result<(), super::AppendUpdateError> {
        debug_assert!(line.ends_with(b"\n"), "JSONL record must end with \\n");
        let lock = Self::lock_append(path).map_err(super::AppendUpdateError::NotCommitted)?;
        let result = (|| {
            let mut file = OpenOptions::new()
                .read(true)
                .create(true)
                .append(true)
                .open(path)
                .map_err(super::AppendUpdateError::NotCommitted)?;
            let len = file
                .metadata()
                .map_err(super::AppendUpdateError::NotCommitted)?
                .len();
            if len > 0 {
                file.seek(io::SeekFrom::Start(len - 1))
                    .map_err(super::AppendUpdateError::NotCommitted)?;
                let mut last = [0u8; 1];
                file.read_exact(&mut last)
                    .map_err(super::AppendUpdateError::NotCommitted)?;
                if last[0] != b'\n' {
                    line.insert(0, b'\n');
                }
            }
            file.write_all(&line)
                .map_err(super::AppendUpdateError::NotCommitted)?;
            file.flush()
                .map_err(super::AppendUpdateError::Indeterminate)?;
            sync_file(&file).map_err(super::AppendUpdateError::Indeterminate)?;
            drop(file);
            sync_parent().map_err(super::AppendUpdateError::Indeterminate)?;
            Ok(())
        })();
        let _ = lock.unlock();
        result
    }
    fn append_jsonl_line_sync_with(
        path: &Path,
        mut line: Vec<u8>,
        durability: AppendDurability,
        mut sync_file: impl FnMut(&std::fs::File) -> io::Result<()>,
        mut sync_parent: impl FnMut() -> io::Result<()>,
    ) -> io::Result<()> {
        debug_assert!(line.ends_with(b"\n"), "JSONL record must end with \\n");
        let lock = Self::lock_append(path)?;
        let result = (|| {
            let mut file = OpenOptions::new()
                .read(true)
                .create(true)
                .append(true)
                .open(path)?;
            let len = file.metadata()?.len();
            if len > 0 {
                file.seek(io::SeekFrom::Start(len - 1))?;
                let mut last = [0u8; 1];
                file.read_exact(&mut last)?;
                if last[0] != b'\n' {
                    tracing::warn!(
                        path = %path.display(),
                        "jsonl file has a torn trailing line (previous append crashed mid-write?); terminating it before appending"
                    );
                    line.insert(0, b'\n');
                }
            }
            file.write_all(&line)?;
            file.flush()?;
            if matches!(durability, AppendDurability::Durable) {
                sync_file(&file)?;
                drop(file);
                sync_parent()?;
            } else {
                drop(file);
            }
            Ok(())
        })();
        let _ = lock.unlock();
        result
    }
    async fn append_cwd_switch_with_bookkeeping(
        &self,
        info: &Info,
        message: &ConversationItem,
    ) -> Result<StrictAppendAck, super::AppendCwdSwitchError> {
        let path = self.chat_file(info);
        let mut line = serde_json::to_vec(message).map_err(|error| {
            super::AppendCwdSwitchError::NotCommitted(io::Error::new(
                io::ErrorKind::InvalidData,
                error,
            ))
        })?;
        line.push(b'\n');
        let generation = message
            .working_directory_switch_generation()
            .filter(|generation| *generation > 0)
            .ok_or_else(|| {
                super::AppendCwdSwitchError::NotCommitted(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "working-directory switch item must carry a nonzero generation",
                ))
            })?;
        let disposition = tokio::task::spawn_blocking(move || {
            Self::append_cwd_switch_line_sync_with(
                &path,
                line,
                generation,
                Self::sync_file_durable,
                || Self::sync_parent_directory(&path),
            )
        })
        .await
        .map_err(|error| super::AppendCwdSwitchError::NotCommitted(io::Error::other(error)))??;
        self.apply_summary_patch(
            info,
            super::summary_write::SummaryPatch {
                record_activity: matches!(&disposition, StrictAppendAck::Appended),
                chat_messages: matches!(&disposition, StrictAppendAck::Appended)
                    .then_some(super::summary_write::CounterOp::Increment(1)),
                chat_format_version: Some(CHAT_FORMAT_VERSION),
                cwd_switch_bookkeeping_generation: Some(generation),
                ..Default::default()
            },
        )
        .await
        .map_err(|source| super::AppendCwdSwitchError::Committed {
            acknowledgement: disposition.clone(),
            source,
        })?;
        Self::sync_file_path_durable(self.summary_file(info))
            .await
            .map_err(|source| super::AppendCwdSwitchError::Committed {
                acknowledgement: disposition.clone(),
                source,
            })?;
        Ok(disposition)
    }
    fn find_cwd_switch_generation(
        path: &Path,
        generation: u64,
    ) -> io::Result<Option<ConversationItem>> {
        let contents = match std::fs::read(path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        Ok(contents.split(|byte| *byte == b'\n').find_map(|line| {
            let item = serde_json::from_slice::<ConversationItem>(line).ok()?;
            (item.working_directory_switch_generation() == Some(generation)).then_some(item)
        }))
    }
    pub(crate) fn append_cwd_switch_line_sync_with(
        path: &Path,
        mut line: Vec<u8>,
        generation: u64,
        mut sync_file: impl FnMut(&std::fs::File) -> io::Result<()>,
        mut sync_parent: impl FnMut() -> io::Result<()>,
    ) -> Result<StrictAppendAck, super::AppendCwdSwitchError> {
        let lock = Self::lock_append(path).map_err(super::AppendCwdSwitchError::NotCommitted)?;
        let result = (|| {
            if let Some(authoritative) = Self::find_cwd_switch_generation(path, generation)
                .map_err(super::AppendCwdSwitchError::NotCommitted)?
            {
                return Ok(StrictAppendAck::AlreadyPresent(authoritative));
            }
            let mut file = OpenOptions::new()
                .read(true)
                .create(true)
                .append(true)
                .open(path)
                .map_err(super::AppendCwdSwitchError::NotCommitted)?;
            let len = file
                .metadata()
                .map_err(super::AppendCwdSwitchError::NotCommitted)?
                .len();
            if len > 0 {
                file.seek(io::SeekFrom::Start(len - 1))
                    .map_err(super::AppendCwdSwitchError::NotCommitted)?;
                let mut last = [0u8; 1];
                file.read_exact(&mut last)
                    .map_err(super::AppendCwdSwitchError::NotCommitted)?;
                if last[0] != b'\n' {
                    line.insert(0, b'\n');
                }
            }
            file.write_all(&line)
                .map_err(super::AppendCwdSwitchError::NotCommitted)?;
            file.flush()
                .map_err(|source| super::AppendCwdSwitchError::Committed {
                    acknowledgement: StrictAppendAck::Appended,
                    source,
                })?;
            sync_file(&file).map_err(|source| super::AppendCwdSwitchError::Committed {
                acknowledgement: StrictAppendAck::Appended,
                source,
            })?;
            drop(file);
            sync_parent().map_err(|source| super::AppendCwdSwitchError::Committed {
                acknowledgement: StrictAppendAck::Appended,
                source,
            })?;
            Ok(StrictAppendAck::Appended)
        })();
        let _ = lock.unlock();
        result
    }
    /// Lock tail healing, append, and barriers through `<target>.jsonl.lock`.
    /// Full-file [`Self::write_jsonl`] atomic-rename rewrites bypass this append-only lock.
    fn lock_append(path: &Path) -> io::Result<std::fs::File> {
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.with_extension("jsonl.lock"))?;
        lock.lock_exclusive()?;
        Ok(lock)
    }
    fn sync_file_durable(file: &std::fs::File) -> io::Result<()> {
        super::sync_file_durable(file)
    }
    fn sync_parent_directory(path: &Path) -> io::Result<()> {
        super::sync_parent_directory(path)
    }
    fn backup_chat_history_locked(path: &Path) -> io::Result<()> {
        let backup = path.with_extension("jsonl.pre-strip");
        if !path.exists() {
            return Ok(());
        }
        match std::fs::symlink_metadata(&backup) {
            Ok(metadata) if metadata.file_type().is_file() => return Ok(()),
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "pre-strip backup is not a regular file: {}",
                        backup.display()
                    ),
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }

        let bytes = std::fs::read(path)?;
        let staging = super::temp_sibling(&backup);
        let write_result = (|| {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&staging)?;
            file.write_all(&bytes)?;
            file.flush()?;
            file.sync_all()?;
            drop(file);

            // Linking a fully synced staging inode creates the final name
            // atomically without replace semantics. Even a writer that ignores
            // our sidecar lock cannot overwrite the first backup.
            match std::fs::hard_link(&staging, &backup) {
                Ok(()) => Self::sync_parent_directory(&backup),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    match std::fs::symlink_metadata(&backup) {
                        Ok(metadata) if metadata.file_type().is_file() => Ok(()),
                        Ok(_) => Err(io::Error::new(
                            io::ErrorKind::AlreadyExists,
                            format!(
                                "pre-strip backup is not a regular file: {}",
                                backup.display()
                            ),
                        )),
                        Err(error) => Err(error),
                    }
                }
                Err(error) => Err(error),
            }
        })();
        let _ = std::fs::remove_file(&staging);
        write_result
    }
    /// Write a full JSONL file crash-atomically while holding the same sidecar
    /// lock used by appenders. This prevents a reconnecting writer from
    /// appending between a pre-strip backup and an atomic history rewrite.
    async fn write_jsonl<T: serde::Serialize>(&self, path: PathBuf, items: &[T]) -> io::Result<()> {
        let bytes = super::to_jsonl_bytes(items)?;
        tokio::task::spawn_blocking(move || {
            let lock = Self::lock_append(&path)?;
            let result = super::write_bytes_atomic(&path, &bytes);
            let _ = lock.unlock();
            result
        })
        .await
        .map_err(io::Error::other)?
    }
    fn read_jsonl<T: serde::de::DeserializeOwned>(&self, path: PathBuf) -> io::Result<Vec<T>> {
        if !path.exists() {
            return Ok(Vec::new());
        }
        let mut file = OpenOptions::new().read(true).open(&path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let mut items = Vec::new();
        for line in contents.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let item: T = serde_json::from_str(line)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            items.push(item);
        }
        Ok(items)
    }
    /// Append a session update to the updates.jsonl file, wrapping it in an envelope with timestamp.
    pub(super) async fn append_update_to_file(
        &self,
        path: PathBuf,
        update: &super::SessionUpdate,
        durability: AppendDurability,
    ) -> io::Result<()> {
        #[cfg(test)]
        if let Some(append_probe) = &self.update_append_probe {
            append_probe(durability)?;
        }
        let envelope = SessionUpdateEnvelope::from_update(update)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let mut line = serde_json::to_vec(&envelope)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        line.push(b'\n');
        Self::append_jsonl_line_blocking(path, line, durability).await
    }
    async fn append_update_with_bookkeeping(
        &self,
        info: &Info,
        update: &super::SessionUpdate,
        durability: AppendDurability,
    ) -> Result<(), super::AppendUpdateError> {
        let updates_path = self.updates_file(info);
        if matches!(durability, AppendDurability::Durable) {
            let envelope = SessionUpdateEnvelope::from_update(update).map_err(|error| {
                super::AppendUpdateError::NotCommitted(io::Error::new(
                    io::ErrorKind::InvalidData,
                    error,
                ))
            })?;
            let mut line = serde_json::to_vec(&envelope).map_err(|error| {
                super::AppendUpdateError::NotCommitted(io::Error::new(
                    io::ErrorKind::InvalidData,
                    error,
                ))
            })?;
            line.push(b'\n');
            #[cfg(test)]
            if let Some(append_probe) = &self.update_append_probe {
                append_probe(durability).map_err(super::AppendUpdateError::NotCommitted)?;
            }
            tokio::task::spawn_blocking(move || {
                Self::append_durable_jsonl_line_commit_aware(&updates_path, line)
            })
            .await
            .map_err(|error| super::AppendUpdateError::NotCommitted(io::Error::other(error)))??;
        } else {
            self.append_update_to_file(updates_path, update, durability)
                .await
                .map_err(super::AppendUpdateError::NotCommitted)?;
        }
        self.apply_summary_patch(
            info,
            super::summary_write::SummaryPatch {
                record_activity: true,
                messages: Some(super::summary_write::CounterOp::Increment(1)),
                ..Default::default()
            },
        )
        .await
        .map_err(super::AppendUpdateError::Committed)
    }
    /// Read session updates from an updates.jsonl file, handling both envelope and legacy formats.
    ///
    /// Uses direct string-to-typed deserialization (via `SessionUpdateEnvelope::from_str`)
    /// with a borrowing envelope and `&RawValue` to avoid intermediate `Value` allocation.
    ///
    /// Corruption-tolerant like [`Self::read_chat_history_sync`]: updates are
    /// display/replay data appended non-atomically, so a torn line (crashed or
    /// racing append) is skipped with a warning instead of failing the caller
    /// (session load, fork copy). The live replay path is already lenient;
    /// this keeps the fork path from bricking on the same corruption.
    fn read_updates_jsonl(&self, path: PathBuf) -> io::Result<Vec<super::SessionUpdate>> {
        if !path.exists() {
            return Ok(Vec::new());
        }
        let contents = std::fs::read(&path)?;
        let mut skipped_lines: usize = 0;
        let mut updates = Vec::new();
        for line in contents.split(|b| *b == b'\n') {
            let line = line.trim_ascii();
            if line.is_empty() {
                continue;
            }
            let parsed = std::str::from_utf8(line)
                .map_err(|e| e.to_string())
                .and_then(|s| SessionUpdateEnvelope::from_str(s).map_err(|e| e.to_string()));
            match parsed {
                Ok(update) => updates.push(update),
                Err(error) => {
                    skipped_lines += 1;
                    if skipped_lines == 1 {
                        tracing::warn!(
                            error = %error,
                            path = %path.display(),
                            "skipping unparseable updates.jsonl line (torn append?)"
                        );
                    }
                }
            }
        }
        if skipped_lines > 0 {
            tracing::warn!(
                skipped = skipped_lines,
                loaded = updates.len(),
                path = %path.display(),
                "skipped unparseable session update lines"
            );
        }
        Ok(updates)
    }
    /// Write summary via the identity journal (Leave when a pair exists so
    /// digests stay aligned; plain summary stage otherwise). Session dirs are
    /// created owner-only (`0700`) so older `0755` layouts retighten on write.
    fn write_summary_sync(&self, info: &Info, summary: &Summary) -> io::Result<()> {
        let session_dir = self.create_session_dir_owner_only(info)?;
        super::model_route::commit_summary_and_companion(&session_dir, summary, None, true)
    }
    /// Dirfd-relative summary read via the multi-component trusted-root walk.
    ///
    /// Containment failures (ELOOP on intermediate `sessions` symlink, owner
    /// mismatch, TOCTOU revalidate) fail closed. There is **no** path-follow
    /// fallback — a walk error must never adopt attacker content via
    /// `std::fs::read`.
    fn read_summary_sync(&self, info: &Info) -> io::Result<Summary> {
        let session_dir = self.session_dir(info);
        super::model_route::read_summary_contained(&session_dir)
    }
    fn read_optional_json_sync<T: serde::de::DeserializeOwned>(
        &self,
        path: &Path,
    ) -> io::Result<Option<T>> {
        if !path.exists() {
            return Ok(None);
        }
        match std::fs::read_to_string(path) {
            Ok(s) if s.trim().is_empty() => Ok(None),
            Ok(s) => match serde_json::from_str::<T>(&s) {
                Ok(v) => Ok(Some(v)),
                Err(e) => {
                    tracing::warn!(?e, "failed parsing json; returning None");
                    Ok(None)
                }
            },
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    tracing::warn!(?e, "failed reading json; returning None");
                }
                Ok(None)
            }
        }
    }
    fn load_workflow_runs_sync(
        &self,
        info: &Info,
    ) -> io::Result<Vec<crate::session::workflow::store::RestoredWorkflowRun>> {
        use crate::session::workflow::store::{
            MAX_RESTORED_WORKFLOW_RUNS, MAX_WORKFLOW_ARGS_BYTES, MAX_WORKFLOW_MANIFEST_BYTES,
            read_bounded_nofollow,
        };
        let workflows_dir = self.workflows_dir(info);
        match std::fs::symlink_metadata(&workflows_dir) {
            Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
                return Ok(Vec::new());
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(Vec::new());
            }
            Err(error) => return Err(error),
        };
        let mut entries: Vec<_> = std::fs::read_dir(&workflows_dir)?
            .filter_map(Result::ok)
            .take(MAX_RESTORED_WORKFLOW_RUNS.saturating_add(1))
            .collect();
        let entries_truncated = entries.len() > MAX_RESTORED_WORKFLOW_RUNS;
        entries.truncate(MAX_RESTORED_WORKFLOW_RUNS);
        entries.sort_by_key(|entry| entry.file_name());
        if entries_truncated {
            tracing::warn!(
                path = %workflows_dir.display(),
                limit = MAX_RESTORED_WORKFLOW_RUNS,
                "workflow restore run-count cap reached; ignoring remaining entries"
            );
        }
        let mut restored = Vec::new();
        for entry in entries {
            let run_dir = entry.path();
            let Ok(run_meta) = std::fs::symlink_metadata(&run_dir) else {
                continue;
            };
            if run_meta.file_type().is_symlink() || !run_meta.is_dir() {
                continue;
            }
            if std::fs::symlink_metadata(run_dir.join("cleared"))
                .is_ok_and(|meta| meta.is_file() && !meta.file_type().is_symlink())
            {
                continue;
            }
            let manifest_path = run_dir.join("state.json");
            let manifest = match read_bounded_nofollow(&manifest_path, MAX_WORKFLOW_MANIFEST_BYTES)
                .and_then(|bytes| {
                    serde_json::from_slice::<crate::session::workflow::store::WorkflowRunManifest>(
                        &bytes,
                    )
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
                }) {
                Ok(manifest) => manifest,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => {
                    tracing::warn!(path = %manifest_path.display(), %error, "skipping invalid workflow manifest");
                    continue;
                }
            };
            if !matches!(
                manifest.version,
                1..=crate::session::workflow::store::WORKFLOW_RUN_MANIFEST_VERSION
            ) || crate::session::workflow::store::validate_run_id(&manifest.state.run_id)
                .is_err()
                || run_dir.file_name().and_then(|name| name.to_str())
                    != Some(manifest.state.run_id.as_str())
            {
                tracing::warn!(path = %manifest_path.display(), "skipping unsupported or mismatched workflow manifest");
                continue;
            }
            let script_path = crate::session::workflow::store::script_revision_path(
                &run_dir,
                manifest.script_revision,
            );
            let script = match read_bounded_nofollow(
                &script_path,
                crate::session::workflow::registry::MAX_WORKFLOW_SOURCE_BYTES,
            )
            .and_then(|bytes| {
                String::from_utf8(bytes)
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
            }) {
                Ok(script) => script,
                Err(error) => {
                    tracing::warn!(path = %script_path.display(), %error, "skipping workflow with missing immutable script");
                    continue;
                }
            };
            let args_path = run_dir.join("args.json");
            let args = match read_bounded_nofollow(&args_path, MAX_WORKFLOW_ARGS_BYTES).and_then(
                |bytes| {
                    serde_json::from_slice(&bytes)
                        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
                },
            ) {
                Ok(args) => args,
                Err(error) => {
                    tracing::warn!(path = %args_path.display(), %error, "skipping workflow with missing immutable args");
                    continue;
                }
            };
            restored.push(crate::session::workflow::store::RestoredWorkflowRun {
                manifest,
                script,
                args,
            });
        }
        Ok(restored)
    }
    /// Read chat history from JSONL file, handling both legacy ChatRequestMessage format
    /// (version 0) and new ConversationItem format (version >= 1).
    ///
    /// Uses line-by-line format detection with fallback to handle mixed-format files
    /// that can occur when continuing an old session with a newer binary.
    ///
    /// ## Corruption tolerance (torn / interleaved appends)
    ///
    /// Appends to `chat_history.jsonl` are not crash-atomic: a process kill
    /// mid-append (auto-update leader relaunch), `ENOSPC`, or two writers
    /// racing (a second persistence actor on reconnect) can leave a torn or
    /// merged line — the classic symptom is a serde error like
    /// ``expected `,` or `}` at line 1 column 571``. Failing the whole load on
    /// one bad line bricks the session forever ("Couldn't load session:
    /// FS_OTHER"), which is strictly worse than resuming without the damaged
    /// record. Unparseable / undecodable lines are therefore *skipped* with a
    /// warning, and the first time corruption is detected the raw file is
    /// preserved as `chat_history.jsonl.corrupt` next to the original — the
    /// post-load snapshot rewrite (`persist_chat_history_jsonl_sync`) scrubs
    /// the bad lines from the live file, so the quarantine copy is the only
    /// surviving evidence for debugging / manual recovery.
    ///
    /// Lines are split on raw `\n` bytes and parsed with `from_slice` so a
    /// write torn mid-UTF-8-codepoint poisons only its own line, not the
    /// whole-file `read_to_string`.
    ///
    /// ## Legacy reasoning reconstruction (in-memory upgrade)
    ///
    /// Older sessions stored reasoning either inline on the
    /// assistant (`AssistantItem.reasoning`) or, for early
    /// backend-search sessions, as `AssistantItem.raw_output: Vec<Value>`.
    /// Newer sessions don't have those fields on `AssistantItem` so serde
    /// would silently drop them. We pre-extract them via
    /// [`xai_grok_inference_types::upgrade_legacy_reasoning`] and emit
    /// sibling `Reasoning` / `BackendToolCall` items *before* the
    /// corresponding assistant — matching the order
    /// `response_to_conversation_items` would produce. The file on disk
    /// is not rewritten; this is a load-time-only transform so resumed
    /// sessions get sibling-shape replay without any disk-write risk.
    /// Idempotent: newer sessions have no `reasoning` / `raw_output` /
    /// `reasoning_content` fields, so the upgrader produces no siblings.
    /// The upgrader runs only for lines that decode successfully, so a
    /// skipped corrupt line never emits orphaned siblings or pollutes the
    /// sibling-dedup set.
    fn read_chat_history_sync(
        &self,
        path: PathBuf,
        chat_format_version: u8,
    ) -> io::Result<Vec<ConversationItem>> {
        let Some(mut file) = open_regular_file_nofollow(&path)? else {
            return match std::fs::symlink_metadata(&path) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
                Ok(_) => Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "chat history is not a real regular file: {}",
                        path.display()
                    ),
                )),
                Err(error) => Err(error),
            };
        };
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)?;
        let mut sibling_btc_ids_seen: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let mut upgraded_reasoning_count: usize = 0;
        let mut upgraded_btc_count: usize = 0;
        let mut skipped_lines: usize = 0;
        let mut first_skipped: Option<(usize, String)> = None;
        let mut skip_line = |line_no: usize, error: String| {
            skipped_lines += 1;
            if first_skipped.is_none() {
                first_skipped = Some((line_no, error));
            }
        };
        let mut items = Vec::new();
        for (line_idx, line) in contents.split(|b| *b == b'\n').enumerate() {
            let line = line.trim_ascii();
            if line.is_empty() {
                continue;
            }
            let raw: serde_json::Value = match serde_json::from_slice(line) {
                Ok(raw) => raw,
                Err(e) => {
                    skip_line(line_idx + 1, e.to_string());
                    continue;
                }
            };
            let item_result = if chat_format_version >= CHAT_FORMAT_VERSION {
                serde_json::from_value::<ConversationItem>(raw.clone()).or_else(|e| {
                    serde_json::from_value::<ChatRequestMessage>(raw.clone())
                        .map(ConversationItem::from)
                        .map_err(|_| e)
                })
            } else {
                serde_json::from_value::<ChatRequestMessage>(raw.clone())
                    .map(ConversationItem::from)
                    .or_else(|e| {
                        serde_json::from_value::<ConversationItem>(raw.clone()).map_err(|_| e)
                    })
            };
            let item = match item_result {
                Ok(item) => item,
                Err(e) => {
                    skip_line(line_idx + 1, e.to_string());
                    continue;
                }
            };
            let siblings =
                xai_grok_inference_types::upgrade_legacy_reasoning(&raw, &mut sibling_btc_ids_seen);
            for sib in siblings {
                match &sib {
                    ConversationItem::Reasoning(_) => upgraded_reasoning_count += 1,
                    ConversationItem::BackendToolCall(_) => upgraded_btc_count += 1,
                    _ => {}
                }
                items.push(sib);
            }
            if let ConversationItem::BackendToolCall(b) = &item {
                sibling_btc_ids_seen.insert(b.id().to_string());
            }
            items.push(item);
        }
        let stripped = strip_invalid_images(&mut items);
        // PR19 crash-recovery sanitization: a durable prime <skill_prime>
        // SystemReminder is only ever valid immediately followed by the real
        // user item it was batched with. A torn batch (reminder line flushed,
        // user line lost/partial) would otherwise reload as a lone reminder.
        // Drop only orphan prime reminders (content carries the `<skill_prime>`
        // marker); unrelated standalone SystemReminder items (MCP/plan-mode)
        // without that marker are preserved.
        let dropped_prime = sanitize_orphan_prime_reminders(&mut items);
        if first_skipped.is_some() || stripped > 0 || dropped_prime > 0 {
            let quarantine = path.with_extension("jsonl.corrupt");
            if !quarantine.exists()
                && let Err(e) = std::fs::copy(&path, &quarantine)
            {
                tracing::warn!(
                    error = %e,
                    path = %quarantine.display(),
                    "failed to write chat history quarantine copy"
                );
            }
        }
        if dropped_prime > 0 {
            tracing::warn!(
                count = dropped_prime,
                path = %path.display(),
                "dropped orphan prime system-reminders (torn [reminder, user] \
                 batch); original preserved as *.corrupt"
            );
        }
        if let Some((first_line, first_error)) = first_skipped {
            tracing::warn!(
                skipped = skipped_lines,
                loaded = items.len(),
                first_line,
                first_error = %first_error,
                path = %path.display(),
                "skipped unparseable chat history lines (torn or interleaved \
                 append — crashed mid-write or concurrent writer?); loading \
                 the session without them, original preserved as *.corrupt"
            );
        }
        if stripped > 0 {
            tracing::warn!(
                count = stripped,
                path = %path.display(),
                "stripped invalid images from loaded chat history, original \
                 preserved as *.corrupt"
            );
        }
        if upgraded_reasoning_count > 0 || upgraded_btc_count > 0 {
            tracing::info!(
                upgraded_reasoning = upgraded_reasoning_count,
                upgraded_backend_tool_calls = upgraded_btc_count,
                "reconstructed legacy reasoning siblings from pre-sibling-split session"
            );
        }
        Ok(items)
    }
    /// Apply a typed [`SummaryPatch`](super::summary_write::SummaryPatch) to
    /// this session's `summary.json` under an exclusive sidecar lock, so the
    /// read-modify-write serializes against every other writer (including a
    /// second persistence actor on reconnect, or another process). This is the
    /// only path live sessions use to mutate the summary.
    pub(crate) async fn apply_summary_patch(
        &self,
        info: &Info,
        patch: super::summary_write::SummaryPatch,
    ) -> io::Result<()> {
        self.apply_summary_patch_reporting(info, patch).await?;
        Ok(())
    }
    /// Like [`Self::apply_summary_patch`], but returns whether a
    /// `generated_title_if_absent` was applied (see [`Summary::apply_patch`]).
    async fn apply_summary_patch_reporting(
        &self,
        info: &Info,
        patch: super::summary_write::SummaryPatch,
    ) -> io::Result<bool> {
        let summary_path = self.summary_file(info);
        let lock_path = self.summary_lock_file(info);
        tokio::task::spawn_blocking(move || {
            super::summary_write::apply_patch_locked(&summary_path, &lock_path, &patch)
        })
        .await
        .map_err(io::Error::other)?
    }
}
fn validate_session_id_component(id: &str) -> io::Result<()> {
    let mut components = Path::new(id).components();
    if id.is_empty()
        || id.contains('/')
        || id.contains('\\')
        || id.contains('\0')
        || !matches!(components.next(), Some(Component::Normal(component)) if component == id)
        || components.next().is_some()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "session id must be a non-empty single path component",
        ));
    }
    Ok(())
}

fn is_orchestration_projection_update(update: &super::SessionUpdate) -> bool {
    matches!(
        update,
        super::SessionUpdate::Xai(notification)
            if matches!(
                &notification.update,
                crate::extensions::notification::SessionUpdate::WorkflowUpdated { .. }
                    | crate::extensions::notification::SessionUpdate::GoalUpdated { .. }
            )
    )
}

/// Longest `updates.jsonl` line retained while streaming a fork. A longer line
/// is treated as corruption and drained without ever buffering the transcript.
const MAX_FORK_UPDATE_LINE_BYTES: usize = 64 * 1024 * 1024;

fn for_each_fork_update_line<R: BufRead>(
    reader: R,
    f: impl FnMut(usize, &[u8]) -> io::Result<ControlFlow<()>>,
) -> io::Result<()> {
    for_each_fork_update_line_capped(reader, MAX_FORK_UPDATE_LINE_BYTES, f)
}

fn for_each_fork_update_line_capped<R: BufRead>(
    mut reader: R,
    cap: usize,
    mut f: impl FnMut(usize, &[u8]) -> io::Result<ControlFlow<()>>,
) -> io::Result<()> {
    let mut buffer = Vec::new();
    let mut index = 0usize;
    let mut discarded = 0usize;
    loop {
        buffer.clear();
        let read = reader
            .by_ref()
            .take(cap as u64 + 1)
            .read_until(b'\n', &mut buffer)?;
        if read == 0 {
            break;
        }
        if buffer.len() > cap && buffer.last() != Some(&b'\n') {
            discarded += 1;
            loop {
                buffer.clear();
                let read = reader
                    .by_ref()
                    .take(cap as u64)
                    .read_until(b'\n', &mut buffer)?;
                if read == 0 || buffer.last() == Some(&b'\n') {
                    break;
                }
            }
            continue;
        }
        let line = buffer.trim_ascii();
        if line.is_empty() {
            continue;
        }
        if f(index, line)?.is_break() {
            break;
        }
        index += 1;
    }
    if discarded > 0 {
        tracing::warn!(
            discarded,
            max_bytes = cap,
            "discarded over-long updates.jsonl lines during fork copy"
        );
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum ForkRewindStep {
    Rewind { target: usize },
    UserChunk { prompt_index: Option<usize> },
    Other,
}

fn fork_rewind_step_for_line(line: &str) -> ForkRewindStep {
    let (raw_params, is_xai) =
        if let Ok(envelope) = serde_json::from_str::<super::RawLinePeek<'_>>(line) {
            (
                envelope.params.map(|params| params.get()).unwrap_or(line),
                envelope.method == Some(super::XAI_SESSION_UPDATE_METHOD),
            )
        } else {
            (line, false)
        };
    let Some(update) = serde_json::from_str::<super::RawParamsPeek<'_>>(raw_params)
        .ok()
        .and_then(|params| params.update)
    else {
        return ForkRewindStep::Other;
    };
    if is_xai
        && update.session_update == *crate::session::wire_tags::REWIND_MARKER
        && let Some(target) = update.target_prompt_index
    {
        return ForkRewindStep::Rewind { target };
    }
    let host_turn = update
        .meta
        .as_ref()
        .and_then(|meta| meta.host_turn)
        .unwrap_or(false);
    if !is_xai
        && !host_turn
        && update.session_update == *crate::session::wire_tags::USER_MESSAGE_CHUNK
    {
        return ForkRewindStep::UserChunk {
            prompt_index: update
                .meta
                .as_ref()
                .and_then(|meta| meta.prompt_index.map(|value| value as usize)),
        };
    }
    ForkRewindStep::Other
}

#[derive(serde::Deserialize)]
struct RawSessionIdPeek<'a> {
    #[serde(borrow, rename = "sessionId")]
    session_id: &'a serde_json::value::RawValue,
}

fn raw_subslice_range(container: &str, subslice: &str) -> io::Result<std::ops::Range<usize>> {
    let start = subslice.as_ptr() as usize;
    let base = container.as_ptr() as usize;
    let offset = start.checked_sub(base).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "JSON raw value is outside its container",
        )
    })?;
    let end = offset.checked_add(subslice.len()).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "JSON raw value range overflow")
    })?;
    if end > container.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "JSON raw value is outside its container",
        ));
    }
    Ok(offset..end)
}

fn write_params_with_session_id(
    writer: &mut impl Write,
    raw_params: &str,
    target_session_id: &acp::SessionId,
) -> io::Result<()> {
    let params = serde_json::from_str::<RawSessionIdPeek<'_>>(raw_params)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let range = raw_subslice_range(raw_params, params.session_id.get())?;
    writer.write_all(raw_params[..range.start].as_bytes())?;
    serde_json::to_writer(writer.by_ref(), target_session_id)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    writer.write_all(raw_params[range.end..].as_bytes())
}

#[derive(Default)]
struct CopiedForkUpdates {
    count: usize,
    compaction_checkpoints_copied: usize,
}

struct ForkUpdateWriter<'a> {
    writer: BufWriter<std::fs::File>,
    source: &'a Path,
    source_session_dir: &'a Path,
    staging_dir: &'a Path,
    target_session_id: &'a acp::SessionId,
    copied: CopiedForkUpdates,
    skipped: usize,
}

impl<'a> ForkUpdateWriter<'a> {
    fn new(
        target: &Path,
        source: &'a Path,
        source_session_dir: &'a Path,
        staging_dir: &'a Path,
        target_session_id: &'a acp::SessionId,
    ) -> io::Result<Self> {
        let file = open_private_new_file(target)?;
        Ok(Self {
            writer: BufWriter::new(file),
            source,
            source_session_dir,
            staging_dir,
            target_session_id,
            copied: CopiedForkUpdates::default(),
            skipped: 0,
        })
    }

    fn copy_line(&mut self, line: &[u8]) -> io::Result<()> {
        let line = match std::str::from_utf8(line) {
            Ok(line) => line,
            Err(error) => return self.skip_line(error),
        };
        let update = match SessionUpdateEnvelope::from_str(line) {
            Ok(update) => update,
            Err(error) => return self.skip_line(error),
        };
        if is_orchestration_projection_update(&update) {
            return Ok(());
        }
        if let super::SessionUpdate::Xai(notification) = &update
            && let crate::extensions::notification::SessionUpdate::CompactionCheckpoint(info) =
                &notification.update
        {
            self.copied.compaction_checkpoints_copied += copy_referenced_checkpoint(
                info,
                self.source_session_dir,
                self.staging_dir,
                notification.session_id.0.as_ref(),
            )?;
        }

        let envelope = serde_json::from_str::<super::RawLinePeek<'_>>(line).ok();
        if let Some(raw_params) = envelope.as_ref().and_then(|value| value.params) {
            let params_range = raw_subslice_range(line, raw_params.get())?;
            self.writer
                .write_all(line[..params_range.start].as_bytes())?;
            write_params_with_session_id(
                &mut self.writer,
                raw_params.get(),
                self.target_session_id,
            )?;
            self.writer.write_all(line[params_range.end..].as_bytes())?;
        } else {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_secs())
                .unwrap_or(0);
            write!(
                self.writer,
                "{{\"timestamp\":{timestamp},\"method\":\"session/update\",\"params\":"
            )?;
            write_params_with_session_id(&mut self.writer, line, self.target_session_id)?;
            self.writer.write_all(b"}")?;
        }
        self.writer.write_all(b"\n")?;
        self.copied.count += 1;
        Ok(())
    }

    fn skip_line(&mut self, error: impl std::fmt::Display) -> io::Result<()> {
        self.skipped += 1;
        if self.skipped == 1 {
            tracing::warn!(
                error = %error,
                path = %self.source.display(),
                "skipping unparseable updates.jsonl line during fork copy"
            );
        }
        Ok(())
    }

    fn finish(mut self) -> io::Result<CopiedForkUpdates> {
        if self.skipped > 1 {
            tracing::warn!(
                skipped = self.skipped,
                copied = self.copied.count,
                path = %self.source.display(),
                "skipped unparseable session update lines during fork copy"
            );
        }
        self.writer.flush()?;
        super::sync_file_durable(self.writer.get_ref())?;
        Ok(self.copied)
    }
}

fn read_prompt_offset(index: &mut std::fs::File, prompt: usize) -> io::Result<Option<u64>> {
    let byte_offset = (prompt as u64).checked_mul(8).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "prompt index offset overflow")
    })?;
    if byte_offset.saturating_add(8) > index.metadata()?.len() {
        return Ok(None);
    }
    index.seek(io::SeekFrom::Start(byte_offset))?;
    let mut bytes = [0u8; 8];
    index.read_exact(&mut bytes)?;
    Ok(Some(u64::from_le_bytes(bytes)))
}

fn build_rewind_filtered_scratch(
    source: &mut std::fs::File,
    scratch_path: &Path,
    index_path: &Path,
) -> io::Result<std::fs::File> {
    let mut scratch = open_private_new_read_write_file(scratch_path)?;
    let mut prompt_offsets = open_private_new_read_write_file(index_path)?;
    let mut tracker = super::UserRunTurnTracker::new();
    for_each_fork_update_line(BufReader::new(source), |_, line| {
        let step = std::str::from_utf8(line)
            .map(fork_rewind_step_for_line)
            .unwrap_or(ForkRewindStep::Other);
        match step {
            ForkRewindStep::Rewind { target } => {
                let current_end = scratch.stream_position()?;
                let truncate_to =
                    read_prompt_offset(&mut prompt_offsets, target)?.unwrap_or(current_end);
                scratch.set_len(truncate_to)?;
                scratch.seek(io::SeekFrom::Start(truncate_to))?;
                let retained_offsets = (target as u64)
                    .checked_mul(8)
                    .unwrap_or(u64::MAX)
                    .min(prompt_offsets.metadata()?.len());
                prompt_offsets.set_len(retained_offsets)?;
                prompt_offsets.seek(io::SeekFrom::End(0))?;
                tracker.on_non_user();
                return Ok(ControlFlow::Continue(()));
            }
            ForkRewindStep::UserChunk { prompt_index } => {
                if tracker.on_user_chunk(prompt_index) {
                    prompt_offsets.seek(io::SeekFrom::End(0))?;
                    prompt_offsets.write_all(&scratch.stream_position()?.to_le_bytes())?;
                }
            }
            ForkRewindStep::Other => tracker.on_non_user(),
        }
        scratch.write_all(line)?;
        scratch.write_all(b"\n")?;
        Ok(ControlFlow::Continue(()))
    })?;
    scratch.flush()?;
    scratch.seek(io::SeekFrom::Start(0))?;
    Ok(scratch)
}

fn truncate_scratch_for_prompt(
    scratch: &mut std::fs::File,
    target_prompt_index: usize,
) -> io::Result<()> {
    scratch.seek(io::SeekFrom::Start(0))?;
    let mut tracker = super::UserRunTurnTracker::new();
    let mut turns = 0usize;
    let mut byte_offset = 0u64;
    let mut truncate_to = scratch.metadata()?.len();
    for_each_fork_update_line(BufReader::new(&mut *scratch), |_, line| {
        let step = std::str::from_utf8(line)
            .map(fork_rewind_step_for_line)
            .unwrap_or(ForkRewindStep::Other);
        match step {
            ForkRewindStep::UserChunk { prompt_index } => {
                if tracker.on_user_chunk(prompt_index) {
                    turns += 1;
                    if turns > target_prompt_index + 1 {
                        truncate_to = byte_offset;
                        return Ok(ControlFlow::Break(()));
                    }
                }
            }
            ForkRewindStep::Rewind { .. } | ForkRewindStep::Other => tracker.on_non_user(),
        }
        byte_offset = byte_offset.saturating_add(line.len() as u64 + 1);
        Ok(ControlFlow::Continue(()))
    })?;
    scratch.set_len(truncate_to)?;
    scratch.seek(io::SeekFrom::Start(0))?;
    Ok(())
}

fn copy_referenced_checkpoint(
    info: &crate::extensions::notification::CompactionCheckpointInfo,
    source_session_dir: &Path,
    staging_dir: &Path,
    source_session_id: &str,
) -> io::Result<usize> {
    if super::validate_compaction_checkpoint_id(&info.checkpoint_id).is_err() {
        tracing::warn!(
            checkpoint_id = %info.checkpoint_id,
            session_id = source_session_id,
            "skipping compaction checkpoint with invalid id during copy",
        );
        return Ok(0);
    }
    let relative = Path::new(&info.checkpoint_file);
    let expected_name = format!("{}.json", info.checkpoint_id);
    let mut components = relative.components();
    let well_formed = matches!(
        components.next(),
        Some(Component::Normal(component)) if component == "compaction_checkpoints"
    ) && matches!(
        components.next(),
        Some(Component::Normal(component)) if component == expected_name.as_str()
    ) && components.next().is_none();
    if !well_formed {
        tracing::warn!(
            checkpoint_file = %info.checkpoint_file,
            session_id = source_session_id,
            "skipping compaction checkpoint with unexpected path during copy",
        );
        return Ok(0);
    }

    let source_dir = source_session_dir.join("compaction_checkpoints");
    match std::fs::symlink_metadata(&source_dir) {
        Ok(metadata) if metadata.file_type().is_dir() && !metadata_is_link_like(&metadata) => {}
        Ok(metadata) => {
            tracing::warn!(
                path = %source_dir.display(),
                file_type = ?metadata.file_type(),
                session_id = source_session_id,
                "compaction_checkpoints is not a real directory; skipping checkpoint copy",
            );
            return Ok(0);
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error),
    }

    let source = source_session_dir.join(relative);
    let destination = staging_dir.join(relative);
    if destination.try_exists()? {
        return Ok(0);
    }
    Ok(usize::from(copy_regular_file_atomic(
        &source,
        &destination,
    )?))
}

fn copy_updates_streaming(
    source: &Path,
    target: &Path,
    source_session_dir: &Path,
    staging_dir: &Path,
    target_session_id: &acp::SessionId,
    target_prompt_index: Option<usize>,
) -> io::Result<CopiedForkUpdates> {
    let mut writer = ForkUpdateWriter::new(
        target,
        source,
        source_session_dir,
        staging_dir,
        target_session_id,
    )?;
    let Some(mut source_file) = open_regular_file_nofollow(source)? else {
        return writer.finish();
    };
    match target_prompt_index {
        None => for_each_fork_update_line(BufReader::new(source_file), |_, line| {
            writer.copy_line(line)?;
            Ok(ControlFlow::Continue(()))
        })?,
        Some(target_prompt_index) => {
            let scratch_path = super::temp_sibling(target);
            let index_path = super::temp_sibling(&scratch_path);
            let copy_result = (|| {
                let mut scratch =
                    build_rewind_filtered_scratch(&mut source_file, &scratch_path, &index_path)?;
                truncate_scratch_for_prompt(&mut scratch, target_prompt_index)?;
                for_each_fork_update_line(BufReader::new(scratch), |_, line| {
                    writer.copy_line(line)?;
                    Ok(ControlFlow::Continue(()))
                })
            })();
            let _ = std::fs::remove_file(&scratch_path);
            let _ = std::fs::remove_file(&index_path);
            copy_result?;
        }
    }
    writer.finish()
}

struct SessionCopyStaging {
    path: PathBuf,
    published: bool,
}

impl SessionCopyStaging {
    fn create(target_dir: &Path) -> io::Result<Self> {
        let parent = target_dir.parent().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "target session has no parent")
        })?;
        match std::fs::symlink_metadata(parent) {
            Ok(metadata) if metadata.file_type().is_dir() && !metadata_is_link_like(&metadata) => {}
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "target session parent is not a real directory: {}",
                        parent.display()
                    ),
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                crate::util::grok_home::create_dir_all_owner_only(parent)?;
            }
            Err(error) => return Err(error),
        }
        let target_name = target_dir
            .file_name()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "target session has no file name",
                )
            })?
            .to_string_lossy();
        for _ in 0..16 {
            let path = parent.join(format!(
                ".{target_name}.copy-{}.tmp",
                uuid::Uuid::now_v7().simple()
            ));
            let mut builder = std::fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&path) {
                Ok(()) => {
                    return Ok(Self {
                        path,
                        published: false,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not create a unique session copy staging directory",
        ))
    }

    fn publish(mut self, target_dir: &Path) -> io::Result<()> {
        atomic_rename_directory_noreplace(&self.path, target_dir)?;
        self.published = true;
        super::sync_parent_directory(target_dir)
    }
}

impl Drop for SessionCopyStaging {
    fn drop(&mut self) {
        if !self.published {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

#[cfg(target_os = "linux")]
fn atomic_rename_directory_noreplace(source: &Path, target: &Path) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;

    let source = std::ffi::CString::new(source.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "source path contains NUL"))?;
    let target = std::ffi::CString::new(target.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "target path contains NUL"))?;
    // SAFETY: both pointers are valid NUL-terminated path strings for the
    // duration of the call; AT_FDCWD makes each path relative to cwd as usual.
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            target.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn atomic_rename_directory_noreplace(source: &Path, target: &Path) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;

    let source = std::ffi::CString::new(source.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "source path contains NUL"))?;
    let target = std::ffi::CString::new(target.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "target path contains NUL"))?;
    // SAFETY: both pointers are valid NUL-terminated path strings for the
    // duration of the call. RENAME_EXCL provides no-replace publication.
    let result = unsafe { libc::renamex_np(source.as_ptr(), target.as_ptr(), libc::RENAME_EXCL) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(windows)]
fn atomic_rename_directory_noreplace(source: &Path, target: &Path) -> io::Result<()> {
    // Rust's Windows rename does not replace an existing directory.
    std::fs::rename(source, target)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "ios", windows)))]
fn atomic_rename_directory_noreplace(source: &Path, target: &Path) -> io::Result<()> {
    // There is no portable no-replace rename primitive for directories. On
    // residual targets, preserve fail-closed behavior with a preflight check;
    // publication remains subject to the platform rename race.
    if target.try_exists()? {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("target session already exists: {}", target.display()),
        ));
    }
    std::fs::rename(source, target)
}

fn open_private_new_file(path: &Path) -> io::Result<std::fs::File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    options.open(path)
}

fn open_private_new_read_write_file(path: &Path) -> io::Result<std::fs::File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    options.open(path)
}

fn open_regular_file_nofollow(path: &Path) -> io::Result<Option<std::fs::File>> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // O_NOFOLLOW rejects a final symlink; O_NONBLOCK ensures a FIFO or
        // device substituted before open cannot hang the fork worker.
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = match options.open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        #[cfg(unix)]
        Err(error) if error.raw_os_error() == Some(libc::ELOOP) => return Ok(None),
        Err(error) => return Err(error),
    };
    let metadata = file.metadata()?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Ok(None);
        }
    }
    if !metadata.is_file() {
        return Ok(None);
    }
    Ok(Some(file))
}

fn metadata_is_link_like(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    false
}

fn require_real_directory(path: &Path, description: &str) -> io::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata_is_link_like(&metadata) || !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{description} is not a real directory: {}", path.display()),
        ));
    }
    Ok(())
}

fn copy_regular_file_atomic(source: &Path, destination: &Path) -> io::Result<bool> {
    let Some(mut source_file) = open_regular_file_nofollow(source)? else {
        return Ok(false);
    };
    let parent = destination.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "copy destination has no parent",
        )
    })?;
    crate::util::grok_home::create_dir_all_owner_only(parent)?;
    let staging = super::temp_sibling(destination);
    let result = (|| {
        let mut destination_file = open_private_new_file(&staging)?;
        io::copy(&mut source_file, &mut destination_file)?;
        destination_file.flush()?;
        super::sync_file_durable(&destination_file)?;
        drop(destination_file);
        std::fs::rename(&staging, destination)?;
        super::sync_parent_directory(destination)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&staging);
    }
    result.map(|()| true)
}

fn read_regular_file_nofollow(path: &Path) -> io::Result<Option<Vec<u8>>> {
    let Some(mut file) = open_regular_file_nofollow(path)? else {
        return Ok(None);
    };
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(Some(bytes))
}

/// Every key the `Summary` schema can serialize.
///
/// Serializing a fully populated `Summary` exercises every
/// `skip_serializing_if` predicate, so the key set tracks schema growth
/// automatically: a fork stays authoritative for newly added fields without
/// anyone updating a hand-maintained list.
fn summary_schema_keys() -> std::collections::HashSet<String> {
    let now = chrono::Utc::now();
    let probe = Summary {
        info: Info {
            id: acp::SessionId::new("schema-probe"),
            cwd: "/schema-probe".to_string(),
        },
        cwd_generation: 1,
        previous_cwd: Some("/previous".to_string()),
        pending_cwd_switch_reminder: Some(crate::session::persistence::PendingCwdSwitchReminder {
            cwd_generation: 1,
            previous_cwd: "/previous".to_string(),
            destination_cwd: "/destination".to_string(),
            content: "probe".to_string(),
            destination_project_instructions: Some("instructions".to_string()),
        }),
        cwd_switch_bookkeeping_generation: 1,
        session_summary: "schema probe".to_string(),
        created_at: now,
        updated_at: now,
        num_messages: 1,
        num_chat_messages: 1,
        current_model_id: acp::ModelId::new("schema-probe-model"),
        parent_session_id: Some("schema-probe-parent".to_string()),
        forked_at: Some(now),
        collection_id: Some("schema-probe-collection".to_string()),
        next_trace_turn: 1,
        chat_format_version: CHAT_FORMAT_VERSION,
        prompt_display_cwd: Some("/schema-probe-display".to_string()),
        session_kind: Some("fork".to_string()),
        fork_context_source: Some("new".to_string()),
        fork_parent_prompt_id: Some("schema-probe-prompt".to_string()),
        inherited_prefix_len: Some(1),
        hidden: Some(true),
        source_workspace_dir: Some("/schema-probe-source-workspace".to_string()),
        git_root_dir: Some("/schema-probe-git".to_string()),
        git_remotes: vec!["origin".to_string()],
        head_commit: Some("schema-probe-commit".to_string()),
        head_branch: Some("schema-probe-branch".to_string()),
        request_id: Some("schema-probe-request".to_string()),
        grok_home: Some("/schema-probe-grok-home".to_string()),
        last_active_at: Some(now),
        generated_title: Some("schema probe title".to_string()),
        title_is_manual: true,
        worktree_label: Some("schema-probe-worktree".to_string()),
        agent_name: Some("schema-probe-agent".to_string()),
        sandbox_profile: Some("workspace".to_string()),
        reasoning_effort: Some(xai_grok_inference_types::ReasoningEffort::Medium),
        conversation_language: Some("pt-BR".to_string()),
        execution_backend: crate::agent::execution_backend::ExecutionBackend::ExternalAgent(
            crate::agent::execution_backend::ExternalAgentKind::ClaudeCli,
        ),
        external_runtime: Some(
            crate::agent::external_runtime::ExternalRuntimeEnvelope::for_kind(
                crate::agent::execution_backend::ExternalAgentKind::ClaudeCli,
            ),
        ),
    };
    serde_json::to_value(probe)
        .expect("fully populated Summary must serialize")
        .as_object()
        .expect("Summary must serialize as a JSON object")
        .keys()
        .cloned()
        .collect()
}

fn wrap_fork_summary_json(source: &[u8], target: &Summary) -> io::Result<Vec<u8>> {
    #[derive(serde::Deserialize)]
    struct RawSummaryPeek<'a> {
        #[serde(borrow, default)]
        external_runtime: Option<&'a serde_json::value::RawValue>,
    }

    let raw_external_runtime = serde_json::from_slice::<RawSummaryPeek<'_>>(source)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
        .external_runtime;
    let source_value: serde_json::Value = serde_json::from_slice(source)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let source_object = source_value.as_object().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "source summary must be a JSON object",
        )
    })?;
    let target_value = serde_json::to_value(target)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let target_object = target_value.as_object().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "target summary must be a JSON object",
        )
    })?;
    // The fork target is authoritative for every key in the `Summary` schema:
    // a key the target omitted through a `skip_serializing_if` predicate
    // (for example a cleared `pending_cwd_switch_reminder`) must not leak
    // back from the source. Keys outside the schema carry over from the
    // source so newer summary fields round-trip through older binaries.
    let schema_keys = summary_schema_keys();
    let mut merged = serde_json::Map::new();
    for (key, value) in target_object {
        merged.insert(key.clone(), value.clone());
    }
    for (key, value) in source_object {
        if !merged.contains_key(key) && !schema_keys.contains(key.as_str()) {
            merged.insert(key.clone(), value.clone());
        }
    }

    let mut bytes = Vec::new();
    bytes.push(b'{');
    for (index, (key, value)) in merged.iter().enumerate() {
        if index > 0 {
            bytes.push(b',');
        }
        serde_json::to_writer(&mut bytes, key)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        bytes.push(b':');
        if key == "external_runtime"
            && let Some(raw_external_runtime) = raw_external_runtime
        {
            bytes.extend_from_slice(raw_external_runtime.get().as_bytes());
        } else {
            serde_json::to_writer(&mut bytes, value)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        }
    }
    bytes.push(b'}');
    Ok(bytes)
}

fn open_bounded_media_descriptor(path: &Path) -> io::Result<Option<std::fs::File>> {
    let Some(file) = open_regular_file_nofollow(path)? else {
        tracing::warn!(
            path = %path.display(),
            "media descriptor sidecar is missing or not a real regular file; skipping copy"
        );
        return Ok(None);
    };
    let metadata = file.metadata()?;
    if metadata.len() > crate::session::media_descriptors::MAX_MEDIA_DESCRIPTOR_FILE_BYTES {
        tracing::warn!(
            path = %path.display(),
            size_bytes = metadata.len(),
            "media descriptor sidecar exceeds its copy limit; skipping"
        );
        return Ok(None);
    }
    Ok(Some(file))
}

/// Apply fork-safety filtering to chat history before copying.
///
/// 1. Removes synthetic user messages (doom loop warnings, compaction metadata)
/// 2. Truncates at the last complete turn boundary. A complete turn runs
///    `User → Assistant → (matching ToolResults)`, possibly across multiple
///    Assistant/ToolResult cycles, with `Reasoning` siblings interleaved
///    throughout (real grok-build turns emit `[reasoning, assistant, tool
///    results, reasoning, assistant, ...]`). The scan treats everything
///    except `Assistant` as transparent and only advances the boundary when an
///    Assistant closes every tool call it made, so it survives reasoning
///    interleaving. Trailing incomplete turns — including a trailing
///    user/reasoning tail with no matching assistant response (e.g. the
///    in-flight `/goal` turn) — are removed so the child never sees an
///    incoherent partial turn.
///
/// Also used by the live parent-chat fork path (summarized fallback only — the
/// verbatim mirror path keeps items unfiltered to preserve cached synthetics).
///
/// NOTE: this is one of two reasoning-aware turn-boundary scanners that must move
/// together — the other is `count_complete_turns` in
/// `xai-grok-subagent-resolution/src/context.rs` (it counts turns in the same
/// filtered list during summarization). Keep their notions of a "complete turn"
/// in sync if the turn item model changes.
pub(crate) fn fork_filter_chat(items: &mut Vec<ConversationItem>) {
    items.retain(|item| match item {
        ConversationItem::User(u) => u.synthetic_reason.is_none(),
        _ => true,
    });
    let mut last_complete_end = 0;
    let mut i = 0;
    while i < items.len() {
        match &items[i] {
            ConversationItem::System(_) => {
                last_complete_end = i + 1;
                i += 1;
            }
            ConversationItem::Assistant(asst) => {
                let expected: std::collections::HashSet<&str> =
                    asst.tool_calls.iter().map(|tc| tc.id.as_ref()).collect();
                let mut found = std::collections::HashSet::new();
                let mut j = i + 1;
                while j < items.len() {
                    match &items[j] {
                        ConversationItem::ToolResult(tr) => {
                            if expected.contains(tr.tool_call_id.as_str()) {
                                found.insert(tr.tool_call_id.as_str());
                            }
                            j += 1;
                        }
                        ConversationItem::Reasoning(_) | ConversationItem::BackendToolCall(_) => {
                            j += 1;
                        }
                        _ => break,
                    }
                }
                if found == expected {
                    last_complete_end = j;
                    i = j;
                } else {
                    break;
                }
            }
            _ => {
                i += 1;
            }
        }
    }
    items.truncate(last_complete_end);
}
impl JsonlStorageAdapter {
    /// Fully synchronous version of `copy_session_data` for use inside
    /// `spawn_blocking`. Builds an owner-private sibling staging directory,
    /// durably writes its core files, then atomically publishes it without
    /// replacing an existing target.
    pub fn copy_session_data_sync(
        &self,
        source_info: &Info,
        target_info: &Info,
        options: super::CopySessionOptions,
    ) -> io::Result<super::CopySessionResult> {
        validate_session_id_component(source_info.id.0.as_ref())?;
        validate_session_id_component(target_info.id.0.as_ref())?;
        let source_dir = self.confined_session_dir(source_info)?;
        let target_dir = self.confined_session_dir(target_info)?;
        if let SessionDirMode::FromRoot(root) = &self.dir_mode {
            require_real_directory(&root.join("sessions"), "sessions root")?;
            require_real_directory(
                source_dir.parent().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "source session has no parent")
                })?,
                "source CWD session root",
            )?;
            require_real_directory(&source_dir, "source session directory")?;
        }
        if source_info.id == target_info.id || source_dir == target_dir {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "source and target sessions must differ",
            ));
        }
        if target_dir.try_exists()? {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("target session already exists: {}", target_dir.display()),
            ));
        }
        let source_summary_file = open_regular_file_nofollow(
            &source_dir.join(super::SUMMARY_FILE),
        )?
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "source summary is not a regular file",
            )
        })?;
        let source_summary: Summary = serde_json::from_reader(source_summary_file)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if target_dir.try_exists()? {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("target session already exists: {}", target_dir.display()),
            ));
        }
        if let SessionDirMode::FromRoot(root) = &self.dir_mode {
            crate::util::grok_home::ensure_sessions_cwd_dir_in(root, &target_info.cwd)?;
            require_real_directory(&root.join("sessions"), "sessions root")?;
            require_real_directory(
                target_dir.parent().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "target session has no parent")
                })?,
                "target CWD session root",
            )?;
        }
        let staging = SessionCopyStaging::create(&target_dir)?;
        let staging_adapter = JsonlStorageAdapter::with_explicit_session_dir(staging.path.clone());
        let chat_format_version = source_summary.chat_format_version;
        let mut chat_to_copy: Vec<ConversationItem> = self.read_chat_history_sync(
            source_dir.join(super::CHAT_HISTORY_FILE),
            chat_format_version,
        )?;
        if let Some(target_idx) = options.target_prompt_index {
            // The target prompt is inclusive.
            chat_to_copy.truncate(conversation_truncate_for_prompt(
                &chat_to_copy,
                target_idx + 1,
            ));
        }
        if options.fork_filter {
            fork_filter_chat(&mut chat_to_copy);
        }
        let inherited_prefix_len = if options.fork_filter {
            Some(chat_to_copy.len())
        } else {
            options.inherited_prefix_len
        };
        if !options.skip_cwd_transform && source_info.cwd != target_info.cwd {
            transform_conversation_cwd(&mut chat_to_copy, &source_info.cwd, &target_info.cwd);
        }
        if options.strip_reasoning {
            chat_to_copy = xai_chat_state::compaction_utils::strip_reasoning_blocks(chat_to_copy);
        }
        let num_chat_messages = chat_to_copy.len();
        let cwd_switch_bookkeeping_generation = chat_to_copy
            .iter()
            .filter_map(ConversationItem::working_directory_switch_generation)
            .max()
            .unwrap_or(0);
        // Write and release the bounded chat view before touching the usually
        // much larger update transcript.
        super::write_jsonl_atomic(&staging_adapter.chat_file(target_info), &chat_to_copy)?;
        drop(chat_to_copy);

        let copied_updates = if options.fork_filter {
            super::write_bytes_atomic(&staging_adapter.updates_file(target_info), b"")?;
            CopiedForkUpdates::default()
        } else {
            copy_updates_streaming(
                &source_dir.join(super::UPDATES_FILE),
                &staging_adapter.updates_file(target_info),
                &source_dir,
                &staging.path,
                &target_info.id,
                options.target_prompt_index,
            )?
        };
        let num_messages = copied_updates.count;
        let compaction_checkpoints_copied = copied_updates.compaction_checkpoints_copied;
        let source_summary_raw = read_regular_file_nofollow(&source_dir.join(super::SUMMARY_FILE))?
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "source summary is not a regular file",
                )
            })?;
        let target_model_id = options
            .new_model_id
            .map(acp::ModelId::new)
            .unwrap_or_else(|| source_summary.current_model_id.clone());
        let target_summary = crate::session::persistence::Summary {
            info: target_info.clone(),
            cwd_generation: source_summary.cwd_generation,
            previous_cwd: source_summary.previous_cwd,
            pending_cwd_switch_reminder: None,
            cwd_switch_bookkeeping_generation,
            session_summary: source_summary.session_summary,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            num_messages,
            num_chat_messages,
            current_model_id: target_model_id,
            parent_session_id: options.parent_session_id,
            forked_at: Some(chrono::Utc::now()),
            collection_id: None,
            next_trace_turn: 0,
            chat_format_version: CHAT_FORMAT_VERSION,
            prompt_display_cwd: options.prompt_display_cwd,
            session_kind: Some(options.session_kind.unwrap_or_else(|| "fork".to_string())),
            fork_context_source: options.fork_context_source,
            fork_parent_prompt_id: options.fork_parent_prompt_id,
            inherited_prefix_len,
            hidden: None,
            source_workspace_dir: options.source_workspace_dir,
            git_root_dir: None,
            git_remotes: Vec::new(),
            head_commit: source_summary.head_commit,
            head_branch: source_summary.head_branch,
            request_id: None,
            grok_home: crate::session::persistence::grok_home_string(),
            last_active_at: source_summary.last_active_at,
            generated_title: source_summary.generated_title,
            title_is_manual: source_summary.title_is_manual,
            worktree_label: source_summary.worktree_label,
            agent_name: source_summary.agent_name,
            sandbox_profile: source_summary.sandbox_profile,
            reasoning_effort: source_summary.reasoning_effort,
            conversation_language: source_summary.conversation_language,
            execution_backend: source_summary.execution_backend,
            external_runtime: source_summary.external_runtime,
        };
        // Always wrap the source summary first so unknown top-level keys and
        // raw `external_runtime` survive even when a model_route identity pair
        // is present. Pair copy then rebinds companion/meta to those exact
        // on-disk bytes and must not typed-reserialize the summary.
        let summary_bytes = wrap_fork_summary_json(&source_summary_raw, &target_summary)?;
        super::write_bytes_atomic(&staging_adapter.summary_file(target_info), &summary_bytes)?;
        let models_compatible = target_summary.current_model_id == source_summary.current_model_id;
        if models_compatible {
            match super::model_route::identity_pair_present(&source_dir) {
                Ok(true) => {
                    super::model_route::copy_route_companion_for_fork(
                        &source_dir,
                        &staging.path,
                        &target_summary,
                    )?;
                }
                Ok(false) => {}
                Err(error) => return Err(error),
            }
        }
        let copy_optional_regular =
            |enabled: bool, source: &Path, destination: &Path| -> io::Result<bool> {
                if !enabled {
                    return Ok(false);
                }
                match std::fs::symlink_metadata(source) {
                    Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
                    Err(error) => Err(error),
                    Ok(metadata) if metadata.file_type().is_file() => {
                        copy_regular_file_atomic(source, destination)
                    }
                    Ok(metadata) if metadata_is_link_like(&metadata) => Ok(false),
                    Ok(_) => Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "session sidecar is not a regular file: {}",
                            source.display()
                        ),
                    )),
                }
            };
        let plan_copied = copy_optional_regular(
            options.copy_plan_state,
            &source_dir.join(super::PLAN_FILE),
            &staging_adapter.plan_file(target_info),
        )?;
        let signals_copied = copy_optional_regular(
            options.copy_signals,
            &source_dir.join(super::SIGNALS_FILE),
            &staging_adapter.signals_file(target_info),
        )?;
        let plan_mode_state_copied = copy_optional_regular(
            options.copy_plan_mode_state,
            &source_dir.join(super::PLAN_MODE_FILE),
            &staging_adapter.plan_mode_state_file(target_info),
        )?;
        let tool_state_path = source_dir.join("tool_state.json");
        let tool_state_copied = options.copy_tool_state
            && copy_regular_file_atomic(
                &tool_state_path,
                &staging_adapter
                    .session_dir(target_info)
                    .join("tool_state.json"),
            )?;
        if options.copy_tool_state
            && !tool_state_copied
            && std::fs::symlink_metadata(&tool_state_path)
                .is_ok_and(|metadata| !metadata.file_type().is_file())
        {
            tracing::warn!(
                ?tool_state_path,
                session_id = %source_info.id,
                "tool_state.json is not a real regular file; skipping copy",
            );
        }
        let announcement_state_copied = options.copy_announcement_state
            && copy_regular_file_atomic(
                &source_dir.join(super::ANNOUNCEMENT_STATE_FILE),
                &staging_adapter.announcement_state_file(target_info),
            )?;
        let compaction_segments_copied = if options.copy_compaction_segments {
            let src_dir = source_dir.join(xai_chat_state::compaction_transcript::COMPACTION_DIR);
            let mut copied = 0usize;
            match std::fs::symlink_metadata(&src_dir) {
                Ok(metadata)
                    if metadata.file_type().is_dir() && !metadata_is_link_like(&metadata) =>
                {
                    let dst_dir = staging_adapter
                        .session_dir(target_info)
                        .join(xai_chat_state::compaction_transcript::COMPACTION_DIR);
                    for entry in std::fs::read_dir(&src_dir)? {
                        let entry = entry?;
                        if entry.file_type()?.is_file()
                            && copy_regular_file_atomic(
                                &entry.path(),
                                &dst_dir.join(entry.file_name()),
                            )?
                        {
                            copied += 1;
                        }
                    }
                }
                Ok(metadata) => {
                    tracing::warn!(
                        path = %src_dir.display(),
                        file_type = ?metadata.file_type(),
                        session_id = %source_info.id,
                        "compaction segment source is not a real directory; skipping copy",
                    );
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            copied
        } else {
            0
        };
        let source_session_dir = source_dir;
        let media_descriptor_files_copied = if options.copy_media_descriptors {
            let source =
                source_session_dir.join(crate::session::media_descriptors::MEDIA_DESCRIPTORS_FILE);
            if let Some(source_file) = open_bounded_media_descriptor(&source)? {
                let opened = source_file.metadata()?;
                let destination = staging
                    .path
                    .join(crate::session::media_descriptors::MEDIA_DESCRIPTORS_FILE);
                let mut bytes = Vec::with_capacity(opened.len() as usize);
                std::io::Read::take(
                    source_file,
                    crate::session::media_descriptors::MAX_MEDIA_DESCRIPTOR_FILE_BYTES + 1,
                )
                .read_to_end(&mut bytes)?;
                if bytes.len() as u64
                    > crate::session::media_descriptors::MAX_MEDIA_DESCRIPTOR_FILE_BYTES
                {
                    tracing::warn!(
                        path = %source.display(),
                        session_id = %source_info.id,
                        "media descriptor sidecar grew beyond its copy limit; skipping",
                    );
                    0
                } else {
                    super::write_bytes_atomic(&destination, &bytes)?;
                    1
                }
            } else {
                0
            }
        } else {
            0
        };
        // Forks intentionally do not inherit live workflow or goal projection
        // state. A fresh staging directory makes those paths absent without
        // deleting anything from an existing target.
        debug_assert!(!staging_adapter.workflows_dir(target_info).exists());
        debug_assert!(!staging_adapter.goal_mode_state_file(target_info).exists());
        let result = super::CopySessionResult {
            chat_messages_copied: num_chat_messages,
            updates_copied: num_messages,
            plan_state_copied: plan_copied,
            plan_mode_state_copied,
            signals_copied,
            tool_state_copied,
            announcement_state_copied,
            compaction_segments_copied,
            compaction_checkpoints_copied,
            media_descriptor_files_copied,
        };
        super::sync_parent_directory(&staging.path.join("staged-entry"))?;
        staging.publish(&target_dir)?;
        Ok(result)
    }
}
/// Next `segment_NNN` index in `compaction_dir`: one past the highest existing
/// segment, or 0 when none exist. Resume-safe — derived from disk, not memory.
async fn next_compaction_segment_index(compaction_dir: &std::path::Path) -> u64 {
    let Ok(mut entries) = tokio::fs::read_dir(compaction_dir).await else {
        return 0;
    };
    let mut next = 0u64;
    while let Ok(Some(entry)) = entries.next_entry().await {
        if let Some(n) = entry
            .file_name()
            .to_str()
            .and_then(xai_chat_state::compaction_transcript::parse_segment_index)
        {
            next = next.max(n + 1);
        }
    }
    next
}
#[async_trait]
impl StorageAdapter for JsonlStorageAdapter {
    async fn init_session(&self, info: &Info, model_id: acp::ModelId) -> io::Result<Summary> {
        let dir = self.session_dir(info);
        // Contained existence/load: never Path::exists / tokio::fs::read (those
        // follow a planted intermediate `sessions` symlink). Owner-only dir
        // create/retighten happens only after the walk succeeds, reports
        // NotFound, or reports PermissionDenied on a real directory we own
        // (older grok left `0755`; identity walks refuse group/other bits).
        // ELOOP and other containment failures still fail closed.
        match super::model_route::read_summary_contained(&dir) {
            Ok(summary) => {
                tracing::info!("Loading existing session from JSONL");
                self.create_session_dir_owner_only(info)?;
                Ok(summary)
            }
            Err(e)
                if e.kind() == io::ErrorKind::NotFound
                    || e.kind() == io::ErrorKind::PermissionDenied =>
            {
                let existed = e.kind() == io::ErrorKind::PermissionDenied;
                self.create_session_dir_owner_only(info)?;
                if existed {
                    match super::model_route::read_summary_contained(&dir) {
                        Ok(summary) => {
                            tracing::info!("Loading existing session from JSONL");
                            return Ok(summary);
                        }
                        Err(retry) if retry.kind() == io::ErrorKind::NotFound => {}
                        Err(retry) => return Err(retry),
                    }
                }
                tracing::info!("Creating new session in JSONL");
                let mut summary = Summary::new(info, model_id)?;
                summary.sandbox_profile =
                    xai_grok_sandbox::configured_profile_name().map(String::from);
                self.write_summary_sync(info, &summary)?;
                Ok(summary)
            }
            // ELOOP / TOCTOU / invalid data: fail closed.
            Err(e) => Err(e),
        }
    }
    async fn update_session_title(&self, info: &Info, session_title: String) -> io::Result<()> {
        self.apply_summary_patch(
            info,
            super::summary_write::SummaryPatch {
                generated_title: Some(session_title),
                ..Default::default()
            },
        )
        .await
    }
    async fn set_generated_title_if_absent(
        &self,
        info: &Info,
        session_title: String,
    ) -> io::Result<bool> {
        self.apply_summary_patch_reporting(
            info,
            super::summary_write::SummaryPatch {
                generated_title_if_absent: Some(session_title),
                ..Default::default()
            },
        )
        .await
    }
    async fn append_update(&self, info: &Info, update: &super::SessionUpdate) -> io::Result<()> {
        self.append_update_commit_aware(info, update)
            .await
            .map_err(super::AppendUpdateError::into_io_error)
    }
    async fn append_update_commit_aware(
        &self,
        info: &Info,
        update: &super::SessionUpdate,
    ) -> Result<(), super::AppendUpdateError> {
        self.append_update_with_bookkeeping(info, update, AppendDurability::Buffered)
            .await
    }
    async fn append_update_durable_commit_aware(
        &self,
        info: &Info,
        update: &super::SessionUpdate,
    ) -> Result<(), super::AppendUpdateError> {
        self.append_update_with_bookkeeping(info, update, AppendDurability::Durable)
            .await
    }
    async fn append_chat_message(&self, info: &Info, message: &ConversationItem) -> io::Result<()> {
        self.append_jsonl(self.chat_file(info), message).await?;
        self.apply_summary_patch(
            info,
            super::summary_write::SummaryPatch {
                record_activity: true,
                chat_messages: Some(super::summary_write::CounterOp::Increment(1)),
                chat_format_version: Some(CHAT_FORMAT_VERSION),
                ..Default::default()
            },
        )
        .await
    }
    async fn append_chat_messages(
        &self,
        info: &Info,
        messages: &[ConversationItem],
    ) -> io::Result<()> {
        if messages.is_empty() {
            return Ok(());
        }
        // Serialize the whole batch into ONE buffer and append it in a single
        // sync: a crash either persists the entire `[reminder, user]` pair or
        // a torn tail (healed as one corrupt line), so a prime reminder can
        // never be durably orphaned without its user message.
        let mut buf: Vec<u8> = Vec::new();
        for message in messages {
            buf.extend(
                serde_json::to_vec(message)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?,
            );
            buf.push(b'\n');
        }
        Self::append_jsonl_line_blocking(self.chat_file(info), buf, AppendDurability::Buffered)
            .await?;
        self.apply_summary_patch(
            info,
            super::summary_write::SummaryPatch {
                record_activity: true,
                chat_messages: Some(super::summary_write::CounterOp::Increment(messages.len())),
                chat_format_version: Some(CHAT_FORMAT_VERSION),
                ..Default::default()
            },
        )
        .await
    }
    async fn append_cwd_switch_commit_aware(
        &self,
        info: &Info,
        message: &ConversationItem,
    ) -> Result<StrictAppendAck, super::AppendCwdSwitchError> {
        self.append_cwd_switch_with_bookkeeping(info, message).await
    }
    async fn update_current_model_agent_and_execution(
        &self,
        info: &Info,
        model_id: &acp::ModelId,
        agent_name: Option<&str>,
        reasoning_effort: Option<Option<xai_grok_inference_types::ReasoningEffort>>,
        execution_backend: Option<crate::agent::execution_backend::ExecutionBackend>,
        external_runtime: Option<Option<crate::agent::external_runtime::ExternalRuntimeEnvelope>>,
    ) -> io::Result<()> {
        // Leave-style identity transaction: update model fields and preserve
        // companion when the existing pair is valid.
        self.update_current_model_agent_execution_and_route(
            info,
            model_id,
            agent_name,
            reasoning_effort,
            execution_backend,
            external_runtime,
            None,
        )
        .await
    }

    async fn update_model_route_provenance(
        &self,
        info: &Info,
        provenance: Option<&xai_grok_models::ModelRouteProvenance>,
    ) -> io::Result<()> {
        let session_dir = self.session_dir(info);
        let session_dir2 = session_dir.clone();
        let provenance = provenance.cloned();
        tokio::task::spawn_blocking(move || {
            let summary = super::model_route::read_summary_contained(&session_dir2)?;
            super::model_route::commit_summary_and_companion(
                &session_dir2,
                &summary,
                provenance.as_ref(),
                provenance.is_none(),
            )
        })
        .await
        .map_err(io::Error::other)?
    }

    async fn update_current_model_agent_execution_and_route(
        &self,
        info: &Info,
        model_id: &acp::ModelId,
        agent_name: Option<&str>,
        reasoning_effort: Option<Option<xai_grok_inference_types::ReasoningEffort>>,
        execution_backend: Option<crate::agent::execution_backend::ExecutionBackend>,
        external_runtime: Option<Option<crate::agent::external_runtime::ExternalRuntimeEnvelope>>,
        provenance: Option<&xai_grok_models::ModelRouteProvenance>,
    ) -> io::Result<()> {
        let session_dir = self.session_dir(info);
        let lock_path = self.summary_lock_file(info);
        let model_id = model_id.clone();
        let agent_name = agent_name.map(String::from);
        let provenance = provenance.cloned();
        tokio::task::spawn_blocking(move || {
            // Hold the summary lock across read-modify so concurrent patches serialize,
            // then commit through the identity transaction (summary + companion + meta).
            // Summary bytes are read dirfd-relative — never path-follow.
            let lock = {
                use fs2::FileExt;
                use std::fs::OpenOptions;
                let f = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(false)
                    .open(&lock_path)?;
                f.lock_exclusive()?;
                f
            };
            let mut summary = super::model_route::read_summary_contained(&session_dir)?;
            let patch = super::summary_write::SummaryPatch {
                model: Some(super::summary_write::ModelPatch {
                    model_id,
                    agent_name,
                    reasoning_effort,
                    conversation_language: None,
                    execution_backend,
                    external_runtime,
                }),
                ..Default::default()
            };
            let _ = summary.apply_patch(&patch, chrono::Utc::now());
            let leave = provenance.is_none();
            let result = super::model_route::commit_summary_and_companion(
                &session_dir,
                &summary,
                provenance.as_ref(),
                leave,
            );
            let _ = lock.unlock();
            result
        })
        .await
        .map_err(io::Error::other)?
    }
    async fn update_conversation_language(
        &self,
        info: &Info,
        conversation_language: Option<String>,
    ) -> io::Result<()> {
        self.apply_summary_patch(
            info,
            super::summary_write::SummaryPatch {
                conversation_language: Some(conversation_language),
                ..Default::default()
            },
        )
        .await
    }
    async fn update_collection_id(&self, info: &Info, collection_id: &str) -> io::Result<()> {
        self.apply_summary_patch(
            info,
            super::summary_write::SummaryPatch {
                collection_id: Some(collection_id.to_string()),
                ..Default::default()
            },
        )
        .await
    }
    async fn update_git_head(
        &self,
        info: &Info,
        commit: Option<String>,
        branch: Option<String>,
    ) -> io::Result<()> {
        self.apply_summary_patch(
            info,
            super::summary_write::SummaryPatch {
                git_head: Some(super::summary_write::GitHeadPatch { commit, branch }),
                ..Default::default()
            },
        )
        .await
    }
    async fn update_next_trace_turn(
        &self,
        info: &Info,
        next_trace_turn: u64,
        request_id: Option<&str>,
    ) -> io::Result<()> {
        self.apply_summary_patch(
            info,
            super::summary_write::SummaryPatch {
                trace_turn: Some(super::summary_write::TraceTurnPatch {
                    next_trace_turn,
                    request_id: request_id.map(String::from),
                }),
                ..Default::default()
            },
        )
        .await
    }
    async fn write_plan_state(&self, info: &Info, state: &TodoState) -> io::Result<()> {
        let state_json = serde_json::to_vec_pretty(state)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        tokio::fs::write(self.plan_file(info), state_json).await
    }
    async fn write_plan_mode_state(
        &self,
        info: &Info,
        state: &crate::session::plan_mode::PlanModeSnapshot,
    ) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(state)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        super::write_bytes_atomic_async(&self.plan_mode_state_file(info), json).await
    }
    async fn write_signals(
        &self,
        info: &Info,
        signals: &crate::session::signals::SessionSignals,
    ) -> io::Result<()> {
        let signals_json = serde_json::to_vec(signals)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        super::write_bytes_atomic_async(&self.signals_file(info), signals_json).await
    }
    async fn write_announcement_state(
        &self,
        info: &Info,
        state: &crate::session::announcement_state::AnnouncementState,
    ) -> io::Result<()> {
        let json =
            serde_json::to_vec(state).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        super::write_bytes_atomic_async(&self.announcement_state_file(info), json).await
    }
    async fn write_goal_mode_state(
        &self,
        info: &Info,
        state: &crate::session::goal_tracker::GoalOrchestration,
    ) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(state)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let target = self.goal_mode_state_file(info);
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        super::write_bytes_atomic_async(&target, json).await
    }
    async fn delete_goal_mode_state(&self, info: &Info) -> io::Result<()> {
        match tokio::fs::remove_file(self.goal_mode_state_file(info)).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
    async fn write_workflow_run_state(
        &self,
        info: &Info,
        manifest: &crate::session::workflow::store::WorkflowRunManifest,
    ) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(manifest)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let target = self.workflow_run_state_file(info, &manifest.state.run_id)?;
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await?;
            if parent.join("cleared").is_file() {
                return Ok(());
            }
        }
        if target.is_file()
            && let Ok(existing) = tokio::fs::read(&target).await
            && let Ok(on_disk) = serde_json::from_slice::<
                crate::session::workflow::store::WorkflowRunManifest,
            >(&existing)
            && on_disk.state.run_id == manifest.state.run_id
            && on_disk.state.revision > manifest.state.revision
        {
            tracing::debug!(
                run_id = %manifest.state.run_id,
                on_disk_revision = on_disk.state.revision,
                incoming_revision = manifest.state.revision,
                "skipping stale workflow manifest write"
            );
            return Ok(());
        }
        let tmp = target.with_extension(format!(
            "json.{}.{}.tmp",
            std::process::id(),
            uuid::Uuid::now_v7().simple()
        ));
        tokio::fs::write(&tmp, json).await?;
        #[cfg(windows)]
        match tokio::fs::remove_file(&target).await {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                let _ = tokio::fs::remove_file(&tmp).await;
                return Err(error);
            }
        }
        if let Err(error) = tokio::fs::rename(&tmp, &target).await {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(error);
        }
        Ok(())
    }
    async fn delete_workflow_run_state(&self, info: &Info, run_id: &str) -> io::Result<()> {
        let target = self.workflow_run_state_file(info, run_id)?;
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await?;
            let cleared = parent.join("cleared");
            if !cleared.exists() {
                tokio::fs::write(cleared, []).await?;
            }
        }
        match tokio::fs::remove_file(target).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
    async fn load_session(&self, info: &Info) -> io::Result<PersistedData> {
        let summary = self.read_summary_sync(info)?;
        // Fail closed on mismatched companion/meta; old sessions without pair OK.
        // No rewrite-on-read.
        let session_dir = self.session_dir(info);
        let _ = super::model_route::load_route_companion(&session_dir, &summary)?;
        let chat_file = self.chat_file(info);
        self.ensure_chat_history(info, summary.chat_format_version)?;
        let chat_history = self.read_chat_history_sync(chat_file, summary.chat_format_version)?;
        let updates = self.read_updates_jsonl(self.updates_file(info))?;
        let plan_state = self.read_optional_json_sync::<TodoState>(&self.plan_file(info))?;
        let plan_mode_state = self
            .read_optional_json_sync::<crate::session::plan_mode::PlanModeSnapshot>(
                &self.plan_mode_state_file(info),
            )?;
        let signals = self.read_optional_json_sync::<crate::session::signals::SessionSignals>(
            &self.signals_file(info),
        )?;
        let announcement_state = self
            .read_optional_json_sync::<crate::session::announcement_state::AnnouncementState>(
                &self.announcement_state_file(info),
            )?;
        let goal_mode_state = self
            .read_optional_json_sync::<crate::session::goal_tracker::GoalOrchestration>(
                &self.goal_mode_state_file(info),
            )?;
        let workflow_runs = self.load_workflow_runs_sync(info)?;
        let rewind_points = self.read_jsonl::<RewindPoint>(self.rewind_points_file(info))?;
        let result = PersistedData {
            summary,
            chat_history,
            updates,
            plan_state,
            plan_mode_state,
            rewind_points,
            signals,
            announcement_state,
            goal_mode_state,
            workflow_runs,
        };
        tracing::info!(
            session_id = %info.id,
            num_chat_messages = result.chat_history.len(),
            num_updates = result.updates.len(),
            has_plan = result.plan_state.is_some(),
            has_signals = result.signals.is_some(),
            num_rewind_points = result.rewind_points.len(),
            chat_format_version = result.summary.chat_format_version,
            "Session data loaded successfully from JSONL"
        );
        Ok(result)
    }
    /// Resume path: loads everything except updates and rewind points. Rewind
    /// points can be huge (full file-content snapshots) and are needed only on an
    /// actual rewind, so they're deferred — loaded lazily by `FileStateTracker`.
    async fn load_session_without_updates(
        &self,
        info: &Info,
    ) -> io::Result<super::PersistedDataLight> {
        tracing::info!("Loading session data (without updates) from JSONL");
        let summary = self.read_summary_sync(info)?;
        let session_dir = self.session_dir(info);
        let _ = super::model_route::load_route_companion(&session_dir, &summary)?;
        let chat_file = self.chat_file(info);
        self.ensure_chat_history(info, summary.chat_format_version)?;
        let chat_history = self.read_chat_history_sync(chat_file, summary.chat_format_version)?;
        let plan_state = self.read_optional_json_sync::<TodoState>(&self.plan_file(info))?;
        let plan_mode_state = self
            .read_optional_json_sync::<crate::session::plan_mode::PlanModeSnapshot>(
                &self.plan_mode_state_file(info),
            )?;
        let signals = self.read_optional_json_sync::<crate::session::signals::SessionSignals>(
            &self.signals_file(info),
        )?;
        let announcement_state = self
            .read_optional_json_sync::<crate::session::announcement_state::AnnouncementState>(
                &self.announcement_state_file(info),
            )?;
        let goal_mode_state = self
            .read_optional_json_sync::<crate::session::goal_tracker::GoalOrchestration>(
                &self.goal_mode_state_file(info),
            )?;
        let workflow_runs = self.load_workflow_runs_sync(info)?;
        let result = super::PersistedDataLight {
            summary,
            chat_history,
            plan_state,
            plan_mode_state,
            signals,
            announcement_state,
            goal_mode_state,
            workflow_runs,
        };
        tracing::info!(
            session_id = %info.id,
            num_chat_messages = result.chat_history.len(),
            has_plan = result.plan_state.is_some(),
            has_signals = result.signals.is_some(),
            chat_format_version = result.summary.chat_format_version,
            "Session data loaded (without updates, rewind points deferred) from JSONL"
        );
        Ok(result)
    }
    async fn load_summary(&self, info: &Info) -> io::Result<Summary> {
        let info_clone = info.clone();
        let summary_handle = {
            let info = info_clone.clone();
            let adapter_clone = self.clone();
            tokio::task::spawn_blocking(move || {
                let adapter = adapter_clone;
                adapter.read_summary_sync(&info)
            })
        };
        let summary = summary_handle.await.map_err(io::Error::other)??;
        Ok(summary)
    }
    async fn list_sessions(&self, cwd: Option<&str>) -> io::Result<Vec<Summary>> {
        let adapter = self.clone();
        let cwd = cwd.map(str::to_owned);
        tokio::task::spawn_blocking(move || adapter.list_sessions_sync(cwd.as_deref()))
            .await
            .map_err(io::Error::other)?
    }
    async fn delete_session(&self, info: &Info) -> io::Result<()> {
        let dir = self.session_dir(info);
        match tokio::fs::remove_dir_all(&dir).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }
    async fn append_rewind_point(&self, info: &Info, point: &RewindPoint) -> io::Result<()> {
        self.append_jsonl(self.rewind_points_file(info), point)
            .await
    }
    async fn load_rewind_points(&self, info: &Info) -> io::Result<Vec<RewindPoint>> {
        let info_clone = info.clone();
        let adapter_clone = self.clone();
        tokio::task::spawn_blocking(move || {
            let adapter = adapter_clone;
            let path = adapter.rewind_points_file(&info_clone);
            adapter.read_jsonl::<RewindPoint>(path)
        })
        .await
        .map_err(io::Error::other)?
    }
    async fn truncate_rewind_points_from(&self, info: &Info, from_index: usize) -> io::Result<()> {
        let points = self.load_rewind_points(info).await?;
        let filtered: Vec<RewindPoint> = points
            .into_iter()
            .filter(|p| p.prompt_index < from_index)
            .collect();
        self.write_jsonl(self.rewind_points_file(info), &filtered)
            .await
    }
    async fn merge_rewind_points_from(&self, info: &Info, target_index: usize) -> io::Result<()> {
        let points = self.load_rewind_points(info).await?;
        let merged =
            xai_grok_workspace::session::file_state::merge_rewind_points_from(points, target_index);
        self.write_jsonl(self.rewind_points_file(info), &merged)
            .await
    }
    async fn sync_session_files(&self, info: &Info) -> io::Result<()> {
        let info_clone = info.clone();
        let adapter_clone = self.clone();
        tokio::task::spawn_blocking(move || -> io::Result<()> {
            use std::fs::OpenOptions;
            let adapter = adapter_clone;
            let files_to_sync = [
                adapter.updates_file(&info_clone),
                adapter.chat_file(&info_clone),
                adapter.summary_file(&info_clone),
                adapter.plan_file(&info_clone),
                adapter.rewind_points_file(&info_clone),
            ];
            for file_path in &files_to_sync {
                if file_path.exists()
                    && let Ok(file) = OpenOptions::new().write(true).open(file_path)
                {
                    let _ = file.sync_all();
                }
            }
            Ok(())
        })
        .await
        .map_err(io::Error::other)?
    }
    async fn replace_chat_history(
        &self,
        info: &Info,
        messages: &[ConversationItem],
    ) -> io::Result<()> {
        self.write_jsonl(self.chat_file(info), messages).await?;
        let new_count = messages.len();
        let cwd_switch_bookkeeping_generation = messages
            .iter()
            .filter_map(ConversationItem::working_directory_switch_generation)
            .max()
            .unwrap_or(0);
        self.apply_summary_patch(
            info,
            super::summary_write::SummaryPatch {
                chat_messages: Some(super::summary_write::CounterOp::Set(new_count)),
                chat_format_version: Some(CHAT_FORMAT_VERSION),
                cwd_switch_bookkeeping_generation: Some(cwd_switch_bookkeeping_generation),
                ..Default::default()
            },
        )
        .await
    }
    async fn backup_chat_history_before_strip(&self, info: &Info) -> io::Result<()> {
        let path = self.chat_file(info);
        tokio::task::spawn_blocking(move || {
            // A session can have metadata before its first chat record. Avoid
            // creating a lock sidecar (or failing because its parent does not
            // exist) when there is no history to preserve.
            if !path.try_exists()? {
                return Ok(());
            }
            let lock = Self::lock_append(&path)?;
            let result = Self::backup_chat_history_locked(&path);
            let _ = lock.unlock();
            result
        })
        .await
        .map_err(io::Error::other)?
    }
    async fn replace_chat_history_after_strip(
        &self,
        info: &Info,
        messages: &[ConversationItem],
    ) -> io::Result<()> {
        let path = self.chat_file(info);
        let bytes = super::to_jsonl_bytes(messages)?;
        tokio::task::spawn_blocking(move || {
            let lock = Self::lock_append(&path)?;
            let result = (|| {
                Self::backup_chat_history_locked(&path)?;
                super::write_bytes_atomic(&path, &bytes)
            })();
            let _ = lock.unlock();
            result
        })
        .await
        .map_err(io::Error::other)??;

        let cwd_switch_bookkeeping_generation = messages
            .iter()
            .filter_map(ConversationItem::working_directory_switch_generation)
            .max()
            .unwrap_or(0);
        self.apply_summary_patch(
            info,
            super::summary_write::SummaryPatch {
                chat_messages: Some(super::summary_write::CounterOp::Set(messages.len())),
                chat_format_version: Some(CHAT_FORMAT_VERSION),
                cwd_switch_bookkeeping_generation: Some(cwd_switch_bookkeeping_generation),
                ..Default::default()
            },
        )
        .await
    }
    async fn replace_chat_history_with_compaction_pending(
        &self,
        info: &Info,
        checkpoint_id: &str,
        previous_history: &[ConversationItem],
        compacted_history: &[ConversationItem],
    ) -> Result<(), super::ReplaceCompactionHistoryError> {
        use super::ReplaceCompactionHistoryError;

        let pending = PendingCompactionFile {
            checkpoint_id: checkpoint_id.to_owned(),
            previous_history: previous_history.to_vec(),
            compacted_history_len: compacted_history.len(),
            compacted_history_fingerprint: xai_chat_state::fingerprint_conversation_items(
                compacted_history,
            )
            .map_err(|error| {
                ReplaceCompactionHistoryError::NotCommitted(io::Error::new(
                    io::ErrorKind::InvalidData,
                    error,
                ))
            })?,
        };
        let bytes = serde_json::to_vec_pretty(&pending).map_err(|error| {
            ReplaceCompactionHistoryError::NotCommitted(io::Error::new(
                io::ErrorKind::InvalidData,
                error,
            ))
        })?;
        super::write_bytes_atomic_async(&self.compaction_pending_file(info), bytes)
            .await
            .map_err(ReplaceCompactionHistoryError::NotCommitted)?;

        if let Err(error) = self.replace_chat_history(info, compacted_history).await {
            // `replace_chat_history` can fail after its atomic rename while
            // updating summary bookkeeping. Restore the old cache explicitly
            // before removing the recovery marker so a reported non-commit is
            // always safe for the live actor to continue from.
            if let Err(restore_error) = self.replace_chat_history(info, previous_history).await {
                return Err(ReplaceCompactionHistoryError::Indeterminate(
                    io::Error::other(format!(
                        "history replacement failed ({error}); history rollback failed ({restore_error})"
                    )),
                ));
            }
            if let Err(clear_error) = self.clear_compaction_pending(info).await {
                return Err(ReplaceCompactionHistoryError::Indeterminate(
                    io::Error::other(format!(
                        "history replacement failed ({error}); pending compaction cleanup failed ({clear_error})"
                    )),
                ));
            }
            return Err(ReplaceCompactionHistoryError::NotCommitted(error));
        }
        Ok(())
    }
    async fn clear_compaction_pending(&self, info: &Info) -> io::Result<()> {
        let path = self.compaction_pending_file(info);
        match tokio::fs::remove_file(&path).await {
            Ok(()) => super::sync_parent_directory(&path),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
    async fn copy_session_data(
        &self,
        source_info: &Info,
        target_info: &Info,
        options: super::CopySessionOptions,
    ) -> io::Result<super::CopySessionResult> {
        let storage = self.clone();
        let source = source_info.clone();
        let target = target_info.clone();
        tokio::task::spawn_blocking(move || {
            storage.copy_session_data_sync(&source, &target, options)
        })
        .await
        .map_err(|e| io::Error::other(format!("spawn_blocking panicked: {e}")))?
    }
    async fn load_prompts_only(&self, info: &Info) -> io::Result<Vec<String>> {
        let updates_path = self.updates_file(info);
        if !updates_path.exists() {
            return Ok(Vec::new());
        }
        tokio::task::spawn_blocking(move || {
            let Some(iter) = super::PromptExtractIterator::open(&updates_path)? else {
                return Ok(Vec::new());
            };
            Ok(super::collect_prompts_from_events(iter))
        })
        .await
        .map_err(io::Error::other)?
    }
    #[tracing::instrument(skip_all, fields(session_id = %info.id))]
    async fn load_assistant_text(&self, info: &Info) -> io::Result<Vec<String>> {
        let updates_path = self.updates_file(info);
        if !updates_path.exists() {
            return Ok(Vec::new());
        }
        tokio::task::spawn_blocking(move || {
            let Some(iter) = super::UpdatesIterator::open(&updates_path)? else {
                return Ok(Vec::new());
            };
            Ok(super::collect_assistant_text(iter))
        })
        .await
        .map_err(io::Error::other)?
    }
    #[tracing::instrument(skip_all, fields(session_id = %info.id))]
    async fn load_tool_metadata(&self, info: &Info) -> io::Result<Vec<String>> {
        let updates_path = self.updates_file(info);
        if !updates_path.exists() {
            return Ok(Vec::new());
        }
        tokio::task::spawn_blocking(move || {
            let Some(iter) = super::UpdatesIterator::open(&updates_path)? else {
                return Ok(Vec::new());
            };
            Ok(super::collect_tool_metadata(iter))
        })
        .await
        .map_err(io::Error::other)?
    }
    fn updates_file_path(&self, info: &Info) -> Option<std::path::PathBuf> {
        Some(self.updates_file(info))
    }
    fn rewind_points_file_path(&self, info: &Info) -> Option<std::path::PathBuf> {
        Some(self.rewind_points_file(info))
    }
    async fn append_feedback(
        &self,
        info: &Info,
        entry: &crate::session::persistence::LocalFeedbackEntry,
    ) -> io::Result<()> {
        let path = self.feedback_file(info);
        self.append_jsonl(path, entry).await
    }
    async fn append_btw(
        &self,
        info: &Info,
        entry: &crate::session::persistence::BtwEntry,
    ) -> io::Result<()> {
        let path = self.btw_history_file(info);
        self.append_jsonl(path, entry).await
    }
    async fn write_compaction_checkpoint(
        &self,
        info: &Info,
        checkpoint: &crate::extensions::notification::CompactionCheckpointFile,
    ) -> io::Result<()> {
        super::validate_compaction_checkpoint_id(&checkpoint.checkpoint_id)?;
        let dir = self.session_dir(info).join("compaction_checkpoints");
        tokio::fs::create_dir_all(&dir).await?;
        // A durable marker must never outlive discovery of its checkpoint.
        // Sync the session directory so a newly created checkpoint directory
        // entry is durable; the atomic file write below separately syncs the
        // checkpoint directory after renaming the file into it.
        super::sync_parent_directory(&dir)?;
        let path = dir.join(format!("{}.json", checkpoint.checkpoint_id));
        let bytes = serde_json::to_vec_pretty(checkpoint)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        super::write_bytes_atomic_async(&path, bytes).await
    }
    async fn write_compaction_request(
        &self,
        info: &Info,
        request: &crate::extensions::notification::CompactionRequestFile,
    ) -> io::Result<()> {
        let dir = self.session_dir(info).join("compaction_requests");
        tokio::fs::create_dir_all(&dir).await?;
        let path = dir.join(format!("{}.json", request.request_id));
        let bytes = serde_json::to_vec_pretty(request)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        tokio::fs::write(path, bytes).await
    }
    async fn write_recap_request(
        &self,
        info: &Info,
        request: &crate::extensions::notification::RecapRequestFile,
    ) -> io::Result<()> {
        let dir = self.session_dir(info).join("recap_requests");
        tokio::fs::create_dir_all(&dir).await?;
        let path = dir.join(format!("{}.json", request.request_id));
        let bytes = serde_json::to_vec_pretty(request)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        tokio::fs::write(path, bytes).await
    }
    async fn write_compaction_segment(
        &self,
        info: &Info,
        segment: &crate::extensions::notification::CompactionSegmentFile,
    ) -> io::Result<()> {
        use tokio::io::AsyncWriteExt;
        use xai_chat_state::compaction_transcript::{
            COMPACTION_DIR, INDEX_FILE, INDEX_HEADER, extract_keywords, render_index_row,
            render_segment_md, segment_filename,
        };
        let base = self.session_dir(info).join(COMPACTION_DIR);
        tokio::fs::create_dir_all(&base).await?;
        let index = next_compaction_segment_index(&base).await;
        let md = render_segment_md(
            &segment.items,
            &segment.summary,
            index,
            segment.detail,
            &segment.timestamp,
        );
        tokio::fs::write(base.join(segment_filename(index)), md.as_bytes()).await?;
        let index_path = base.join(INDEX_FILE);
        let needs_header = !tokio::fs::try_exists(&index_path).await.unwrap_or(false);
        let mut f = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&index_path)
            .await?;
        if needs_header {
            f.write_all(INDEX_HEADER.as_bytes()).await?;
        }
        let keywords = extract_keywords(&segment.summary);
        let row = render_index_row(index, segment.items.len(), md.len(), &keywords);
        f.write_all(row.as_bytes()).await?;
        f.flush().await?;
        Ok(())
    }
    async fn read_compaction_checkpoint(
        &self,
        info: &Info,
        checkpoint_file: &str,
    ) -> io::Result<crate::extensions::notification::CompactionCheckpointFile> {
        let path = self.session_dir(info).join(checkpoint_file);
        let bytes = tokio::fs::read(&path).await?;
        serde_json::from_slice(&bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}
/// Max decoded size for a data-URI image loaded from persisted history.
/// Generous (20 MB) — fresh images use 5 MB, but loaded ones just need sanity-checking.
const MAX_LOADED_IMAGE_BYTES: usize = 20 * 1024 * 1024;
/// Strip data-URI images the API would reject (see
/// [`persisted_image_reject_reason`](crate::session::image_normalize::persisted_image_reject_reason):
/// malformed/oversized payloads, truncated or API-rejected formats,
/// dimensions outside the floors/ceiling) from loaded conversation items,
/// so a poisoned history recovers instead of 400ing on every turn.
///
/// User parts become a text placeholder; `ToolResultItem.images` entries
/// are removed. HTTP(S) URLs are left untouched.
///
/// Returns the number of images stripped.
pub(crate) fn strip_invalid_images(items: &mut [ConversationItem]) -> usize {
    fn invalid(part: &ContentPart) -> bool {
        match part {
            ContentPart::Image { url } => url.starts_with("data:") && !is_valid_data_uri_image(url),
            _ => false,
        }
    }
    let mut stripped = 0usize;
    for item in items.iter_mut() {
        match item {
            ConversationItem::User(user) => {
                for part in user.content.iter_mut() {
                    if invalid(part) {
                        *part = ContentPart::Text {
                            text: std::sync::Arc::<str>::from(
                                "[image removed \u{2014} invalid data]",
                            ),
                        };
                        stripped += 1;
                    }
                }
            }
            ConversationItem::ToolResult(t) => {
                let before = t.images.len();
                t.images.retain(|part| !invalid(part));
                stripped += before - t.images.len();
            }
            _ => {}
        }
    }
    stripped
}

/// PR19 crash-recovery sanitization: drop a durable prime `<skill_prime>`
/// `SystemReminder` that is NOT immediately followed by a real user item.
///
/// A healthy prime batch is always `[SystemReminder, real-User]`. If a
/// `ChatBatch` append tears after the reminder line (or mid-user line), reload
/// would otherwise surface a lone durable reminder. Only reminders whose
/// content carries the `<skill_prime>` marker (i.e. the prime reminders, which
/// are only ever written in a batch) are candidates; unrelated standalone
/// `SystemReminder` items (MCP / plan-mode reminders) without that marker are
/// always preserved regardless of position. This is a load-time-only
/// transform; the original file is preserved as `*.corrupt` by the caller.
/// Returns the number of orphan prime reminders dropped.
pub(crate) fn sanitize_orphan_prime_reminders(items: &mut Vec<ConversationItem>) -> usize {
    use xai_grok_inference_types::SyntheticReason;
    let original = std::mem::take(items);
    let mut kept: Vec<ConversationItem> = Vec::with_capacity(original.len());
    let mut dropped = 0usize;
    let mut i = 0usize;
    while i < original.len() {
        let is_prime_reminder = matches!(
            &original[i],
            ConversationItem::User(u)
                if u.synthetic_reason == Some(SyntheticReason::SystemReminder)
                    && original[i].text_content().contains("<skill_prime>")
        );
        let orphan = is_prime_reminder
            && !original.get(i + 1).is_some_and(|next| {
                matches!(
                    next,
                    ConversationItem::User(u) if u.synthetic_reason.is_none()
                )
            });
        if orphan {
            dropped += 1;
        } else {
            kept.push(original[i].clone());
        }
        i += 1;
    }
    *items = kept;
    dropped
}
/// Check that a `data:` URI has a valid `;base64,` header and decodable payload
/// within the size limit.
fn is_valid_data_uri_image(url: &str) -> bool {
    use base64::Engine as _;
    let after_data = match url.strip_prefix("data:") {
        Some(s) => s,
        None => return false,
    };
    let comma = match after_data.find(',') {
        Some(i) => i,
        None => return false,
    };
    let header = &after_data[..comma];
    let payload = &after_data[comma + 1..];
    if !header
        .as_bytes()
        .windows(7)
        .any(|w| w.eq_ignore_ascii_case(b";base64"))
    {
        return false;
    }
    if payload.len() * 3 / 4 > MAX_LOADED_IMAGE_BYTES {
        return false;
    }
    let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(payload) else {
        return false;
    };
    match crate::session::image_normalize::persisted_image_reject_reason(&bytes) {
        None => true,
        Some(reason) => {
            tracing::warn!(reason, "stripping unsendable image from loaded history");
            false
        }
    }
}
#[cfg(test)]
mod durable_tests;
#[cfg(test)]
mod tests;
