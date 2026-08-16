//! Shell-private assigned-spawn transport.
//!
//! This intentionally wraps, rather than changes, the public task event and
//! request shapes. Only `MvpAgent` can mint a sender.

use tokio::sync::mpsc;

use xai_grok_tools::implementations::grok_build::task::types::SubagentRequest;

use super::{assignment::AssignmentKey, exact_route::ExactRoute};

pub(crate) struct InternalAssignedSpawn {
    pub(crate) request: Box<SubagentRequest>,
    pub(crate) key: AssignmentKey,
    pub(crate) route: ExactRoute,
}

#[derive(Clone)]
pub(crate) struct AssignedSpawnSender {
    tx: mpsc::UnboundedSender<InternalAssignedSpawn>,
}

impl AssignedSpawnSender {
    pub(crate) fn send(&self, spawn: InternalAssignedSpawn) -> Result<(), ()> {
        self.tx.send(spawn).map_err(|_| ())
    }
}

pub(crate) fn channel() -> (
    AssignedSpawnSender,
    mpsc::UnboundedReceiver<InternalAssignedSpawn>,
) {
    let (tx, rx) = mpsc::unbounded_channel();
    (AssignedSpawnSender { tx }, rx)
}
