use std::path::Path;

use crate::domain::{
    ApplyReceipt, CoreDistribution, InstallationState, MergeOutcome, RelativePath,
    WorkspaceMutation,
};

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct PortError {
    pub message: String,
}

impl PortError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

pub trait CoreDistributionPort {
    fn current(&self) -> Result<CoreDistribution, PortError>;
}

pub trait InstallationStatePort {
    fn recover_interrupted(&self, root: &Path) -> Result<bool, PortError>;
    fn transaction_pending(&self, root: &Path) -> Result<bool, PortError>;
    fn load(&self, root: &Path) -> Result<Option<InstallationState>, PortError>;
    fn read_workspace_file(
        &self,
        root: &Path,
        path: &RelativePath,
    ) -> Result<Option<Vec<u8>>, PortError>;
    fn validate_managed_path(&self, root: &Path, path: &RelativePath) -> Result<(), PortError>;
    fn apply(
        &self,
        root: &Path,
        state: &InstallationState,
        mutations: &[WorkspaceMutation],
    ) -> Result<ApplyReceipt, PortError>;
}

pub trait ThreeWayMergePort {
    fn available(&self) -> Result<bool, PortError>;
    fn merge(&self, base: &[u8], local: &[u8], upstream: &[u8]) -> Result<MergeOutcome, PortError>;
}
