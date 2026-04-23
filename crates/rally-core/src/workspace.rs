use compact_str::CompactString;
use std::path::PathBuf;

use crate::ids::{Timestamp, WorkspaceId};

#[derive(Debug, Clone)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub name: CompactString,
    pub repo: Option<PathBuf>,
    pub created_at: Timestamp,
    pub archived: bool,
}

impl Workspace {
    pub fn new(id: WorkspaceId, name: CompactString, repo: Option<PathBuf>, at: Timestamp) -> Self {
        Self {
            id,
            name,
            repo,
            created_at: at,
            archived: false,
        }
    }
}
